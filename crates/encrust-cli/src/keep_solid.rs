use anyhow::{Result, bail};
use core_geometry::{Scalar, Vec3};

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
}
