use anyhow::{Context, Result, bail};
use core_geometry::{Bvh, Mesh, Scalar, Transform, Vec3};
use core_volume::{BLOCKER_REACH_MM, Blocker, BlockerSize, blocker_under, rod_on};

use crate::hollowing::polyline;

/// Where the wall stays solid, as the window's Keep solid clicks put it (ADR 0396).
#[derive(Debug, Clone, Default, clap::Args)]
pub struct KeepSolidArgs {
    /// Keep the wall solid at the surface nearest this point, plate mm as --drain-at; more
    /// points, `x,y,z:x,y,z:...`, run a rod from each to the next. Repeat for more.
    #[arg(long = "keep-solid", value_name = "X,Y,Z:...", value_parser = polyline, allow_hyphen_values = true)]
    pub runs: Vec<Vec<Vec3>>,

    /// How far each blocker reaches past the wall into the cavity, millimetres; without it
    /// each fills the model where it lands.
    #[arg(long = "keep-solid-size", value_name = "MM")]
    pub reach_mm: Option<Scalar>,

    /// Keep the wall solid wherever the model is thinner than this many millimetres.
    #[arg(long = "keep-solid-under", value_name = "MM")]
    pub under_mm: Option<Scalar>,
}

impl KeepSolidArgs {
    /// Refuses a size with nothing to size, or one that reaches nowhere.
    pub(crate) fn check_flags(&self, hollowed: bool) -> Result<()> {
        if !self.runs.is_empty() && !hollowed {
            bail!("--keep-solid keeps a shell's wall solid, and --hollow was not given");
        }
        match self.under_mm {
            Some(_) if !hollowed => {
                bail!("--keep-solid-under keeps a shell's wall solid, and --hollow was not given")
            }
            Some(under_mm) if !under_mm.is_finite() || under_mm <= 0.0 => {
                bail!("--keep-solid-under is a thickness, so not {under_mm}")
            }
            _ => {}
        }
        match self.reach_mm {
            Some(_) if self.runs.is_empty() => {
                bail!(
                    "--keep-solid-size sizes the blockers --keep-solid puts down, and none was given"
                )
            }
            Some(reach_mm) if reach_mm.is_nan() || reach_mm <= 0.0 => {
                bail!("--keep-solid-size is how far a blocker reaches, so not {reach_mm}")
            }
            _ => Ok(()),
        }
    }

    /// The blockers on `mesh` as it stands on the plate: a ball at each run's first point and
    /// a rod on to each next, as a click and shift-clicks lay them.
    pub(crate) fn blockers(&self, mesh: &Mesh, bvh: &Bvh) -> Result<Vec<Blocker>> {
        let size = BlockerSize {
            fit: self.reach_mm.is_none(),
            reach_mm: self.reach_mm.unwrap_or(BLOCKER_REACH_MM),
        };
        let mut placed = Vec::new();
        for run in &self.runs {
            let mut last: Option<Blocker> = None;
            for near in run {
                let ball = blocker_near(mesh, bvh, *near, size)
                    .with_context(|| format!("no surface near {near} to keep solid"))?;
                let next = last.map_or(ball, |last| rod_on(&last, &ball));
                placed.push(next);
                last = Some(next);
            }
        }
        Ok(placed)
    }
}

/// The blocker a click on the surface nearest `near` puts down.
fn blocker_near(mesh: &Mesh, bvh: &Bvh, near: Vec3, size: BlockerSize) -> Option<Blocker> {
    let closest = bvh.closest(near)?;
    let normal = mesh.triangle(closest.face)?.normal_unnormalized();
    blocker_under(mesh, bvh, Transform::default(), closest.point, normal, size)
}
