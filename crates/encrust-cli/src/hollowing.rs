use std::fmt;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use core_geometry::{Bvh, Mesh, Scalar, Transform, Vec3};
use core_volume::{
    Cancel, Channel, DrainHole, HoleShape, HoleSize, HollowMode, HollowSettings, InfillPattern,
    InfillSettings, PLUG_GAP_MM, VolumeError, drill, hole_at, hollow, laid_beside, pierce,
    plug_wall_mm, plugs, sleeves,
};

use crate::exit::{Cancelled, Stop};

/// Which surface `--hollow-mode` measures the wall from.
#[derive(Debug, Default, Clone, Copy, clap::ValueEnum, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    #[default]
    Internal,
    External,
}

impl From<Mode> for HollowMode {
    fn from(mode: Mode) -> Self {
        match mode {
            Mode::Internal => Self::Internal,
            Mode::External => Self::External,
        }
    }
}

/// What `--infill` fills the cavity with.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Pattern {
    Hive,
    Grid,
    Scaffold,
}

impl From<Pattern> for InfillPattern {
    fn from(pattern: Pattern) -> Self {
        match pattern {
            Pattern::Hive => Self::Hive,
            Pattern::Grid => Self::Grid,
            Pattern::Scaffold => Self::Scaffold,
        }
    }
}

/// The hollowing half of the command line.
#[derive(Debug, Clone, clap::Args)]
#[command(next_help_heading = "Hollowing")]
pub struct HollowArgs {
    /// Hollow the model, leaving a wall this many millimetres thick.
    #[arg(long = "hollow", value_name = "MM")]
    pub thickness_mm: Option<Scalar>,

    /// Which surface the wall is measured from.
    #[arg(long = "hollow-mode", value_name = "MODE", default_value = "internal")]
    pub mode: Mode,

    /// Open the cavity through the floor the model stands on, so it drains with no hole.
    #[arg(long = "hollow-open-floor")]
    pub open_floor: bool,

    /// Fill the cavity with a lattice.
    #[arg(long = "infill", value_name = "PATTERN")]
    pub infill: Option<Pattern>,

    /// Across one infill cell, millimetres: a hexagon's corners for a hive, a square's
    /// side for a grid, the cube's edge for a scaffold.
    #[arg(long = "infill-cell", value_name = "MM")]
    pub cell_mm: Option<Scalar>,

    /// How much of the cavity the lattice fills, 0 to 1; its walls or struts follow.
    #[arg(long = "infill-density", value_name = "0..1")]
    pub density: Option<Scalar>,

    /// Diameter of the drain holes, millimetres.
    #[arg(long = "drain", value_name = "MM", default_value_t = 3.0)]
    pub drain_mm: Scalar,

    /// Drill a drain hole into the surface nearest this point, in plate millimetres from
    /// the plate's front left corner, after any placement; `inspect` prints a model's own
    /// bounds. Repeat for more than one hole.
    #[arg(long = "drain-at", value_name = "X,Y,Z", value_parser = point, allow_hyphen_values = true)]
    pub drain_at: Vec<Vec3>,

    /// How far past the surface a drain hole reaches, millimetres.
    #[arg(long = "drain-depth", value_name = "MM", default_value_t = 3.0)]
    pub drain_depth_mm: Scalar,

    /// Diameter of a hole's far end as a fraction of its mouth: 1 is a cylinder.
    #[arg(long = "drain-taper", value_name = "0..1", default_value_t = 1.0)]
    pub drain_taper: Scalar,

    /// Cut the drain holes and channels as polygons of this many sides, their corners on the
    /// diameter; round without it.
    #[arg(long = "drain-sides", value_name = "3..12", value_parser = clap::value_parser!(u8).range(3..=12))]
    pub drain_sides: Option<u8>,

    /// Turn a polygonal hole about its axis by this many degrees from the level.
    #[arg(
        long = "drain-turn",
        value_name = "DEG",
        default_value_t = 0.0,
        allow_hyphen_values = true
    )]
    pub drain_turn_deg: Scalar,

    /// Print a plug for each drain hole, standing on the plate beside the model.
    #[arg(long = "drain-plugs")]
    pub plugs: bool,

    /// How much narrower than its hole a plug is drawn, millimetres a side. A resin whose
    /// tolerance compensation is off takes its hole and outer offsets off this too.
    #[arg(long = "plug-gap", value_name = "MM", default_value_t = PLUG_GAP_MM)]
    pub plug_gap_mm: Scalar,

    /// Dig a drainage channel along a polyline of points, `x,y,z:x,y,z:...`, in the same
    /// plate millimetres as --drain-at. Repeat for more than one channel.
    #[arg(long = "channel", value_name = "X,Y,Z:...", value_parser = polyline, allow_hyphen_values = true)]
    pub channels: Vec<Vec<Vec3>>,

    /// Diameter of the drainage channels, millimetres.
    #[arg(long = "channel-size", value_name = "MM", default_value_t = 2.0)]
    pub channel_mm: Scalar,
}

/// One `x,y,z` point of the command line, in plate millimetres.
pub(crate) fn point(text: &str) -> Result<Vec3, String> {
    let numbers: Vec<Scalar> = text
        .split(',')
        .map(|part| {
            part.trim()
                .parse::<Scalar>()
                .map_err(|error| error.to_string())
        })
        .collect::<Result<_, _>>()?;
    match numbers[..] {
        [x, y, z] => Ok(Vec3::new(x, y, z)),
        _ => Err(format!("a point is three numbers, got {text:?}")),
    }
}

/// A polyline of points separated by colons.
fn polyline(text: &str) -> Result<Vec<Vec3>, String> {
    text.split(':').map(point).collect()
}

impl HollowArgs {
    /// Whether a cavity was asked for.
    pub fn wanted(&self) -> bool {
        self.thickness_mm.is_some()
    }

    /// Whether any cut was asked for. A cut does not need a cavity to be made in; see
    /// ADR 0075.
    pub fn cutting(&self) -> bool {
        !self.drain_at.is_empty() || !self.channels.is_empty()
    }

    /// What was asked for, cut on the lattice `precision` picks, or `None` when `--hollow`
    /// was not given. Every channel keeps a wall of solid around it, so it comes out of the
    /// run as a pipe through the part.
    fn settings(&self, precision: Scalar) -> Option<HollowSettings> {
        let thickness_mm = self.thickness_mm?;
        Some(HollowSettings {
            thickness_mm,
            mode: self.mode.into(),
            precision,
            infill: self.infill.map(|pattern| self.infill_settings(pattern)),
            blockers: sleeves(&self.dug(), thickness_mm),
            // The model already stands where it prints, so the plate is straight down.
            open_toward: self.open_floor.then_some(Vec3::NEG_Z),
            ..HollowSettings::default()
        })
    }

    /// Refuses an open floor on anything but a shell, and plugs with no hole to close or a
    /// gap that would make them wider than it.
    pub(crate) fn check_floor_and_plugs(&self) -> Result<()> {
        if self.open_floor && !self.wanted() {
            bail!("--hollow-open-floor opens a shell's floor, and --hollow was not given");
        }
        if self.open_floor && matches!(self.mode, Mode::External) {
            bail!("--hollow-open-floor opens a shell's floor, and a mould has none");
        }
        if self.plugs && self.drain_at.is_empty() {
            bail!("--drain-plugs closes the holes --drain-at drills, and none was given");
        }
        if self.plug_gap_mm.is_nan() || self.plug_gap_mm < 0.0 {
            bail!(
                "--plug-gap is how much narrower a plug is than its hole, so not {}",
                self.plug_gap_mm
            );
        }
        Ok(())
    }

    /// The hole every `--drain-at` drills.
    fn hole_size(&self) -> HoleSize {
        HoleSize {
            diameter_mm: self.drain_mm,
            depth_mm: self.drain_depth_mm,
            taper: self.drain_taper,
            shape: self
                .drain_sides
                .map_or(HoleShape::Round, |sides| HoleShape::Polygon { sides }),
            turn_deg: self.drain_turn_deg,
        }
    }

    /// The lattice `--infill` asked for, every size left out taken from the defaults.
    fn infill_settings(&self, pattern: Pattern) -> InfillSettings {
        let default = InfillSettings::default();
        InfillSettings {
            pattern: pattern.into(),
            cell_mm: self.cell_mm.unwrap_or(default.cell_mm),
            density: self.density.unwrap_or(default.density),
        }
    }

    /// Refuses an infill size given with no infill, rather than dropping it.
    fn check_infill(&self) -> Result<()> {
        let sized = [
            ("--infill-cell", self.cell_mm.is_some()),
            ("--infill-density", self.density.is_some()),
        ];
        match sized.iter().find(|(_, given)| *given) {
            Some((flag, _)) if self.infill.is_none() => {
                bail!("{flag} sizes an infill, and --infill was not given")
            }
            _ => Ok(()),
        }
    }

    fn dug(&self) -> Vec<Channel> {
        self.channels
            .iter()
            .map(|points| Channel {
                points: points.clone(),
                diameter_mm: self.channel_mm,
                shape: self.hole_size().shape,
                turn_deg: self.drain_turn_deg,
            })
            .collect()
    }

    /// A hole on the surface nearest each `--drain-at`, drilled straight into it.
    fn drains(&self, mesh: &Mesh, bvh: &Bvh) -> Result<Vec<DrainHole>> {
        self.drain_at
            .iter()
            .map(|near| {
                hole_at(mesh, bvh, *near, self.hole_size())
                    .with_context(|| format!("no surface near {near} to drill a drain hole into"))
            })
            .collect()
    }
}

/// Hollows the mesh and says what that cost and what it saved. `stop` is asked while the
/// field is built, so a Ctrl-C lands inside the cavity rather than after it.
pub fn run(
    mesh: &Mesh,
    args: &HollowArgs,
    precision: Scalar,
    stop: &Stop,
) -> Result<(Mesh, HollowReport)> {
    let started = Instant::now();
    args.check_infill()?;
    let bvh = Bvh::build(mesh);
    let Some(settings) = args.settings(precision) else {
        bail!("--hollow was not given, so there is nothing to hollow");
    };
    let asked = || stop.requested();
    let hollowed = match hollow(mesh, &bvh, &settings, Cancel::when(&asked)) {
        Err(VolumeError::Cancelled) => return Err(Cancelled.into()),
        other => other.context("hollowing the model")?,
    };

    let report = HollowReport {
        settings,
        faces_before: mesh.faces.len(),
        faces_after: hollowed.mesh.faces.len(),
        cavity_mm3: hollowed.cavity_mm3,
        voxel_mm: hollowed.voxel_mm,
        coarsened: hollowed.coarsened,
        took: started.elapsed(),
    };
    Ok((hollowed.mesh, report))
}

/// Where every `--drain-at` lands on `mesh`, and the channels asked for beside them.
///
/// Placed against the model as it was imported, before any cavity is cut into it, so a
/// point inside the model finds the surface a user was pointing at.
pub fn placed(mesh: &Mesh, args: &HollowArgs) -> Result<(Vec<DrainHole>, Vec<Channel>)> {
    Ok((args.drains(mesh, &Bvh::build(mesh))?, args.dug()))
}

/// A plug for each of `holes` as they are cut, drilled into `mesh` as it stands solid on
/// the plate, laid on the plate in a row past the model's right-hand side. `drift_mm` is
/// what the resin's compensation leaves a fit wider by, and comes off the gap asked for.
pub fn plugs_beside(
    mesh: &Mesh,
    holes: &[DrainHole],
    args: &HollowArgs,
    drift_mm: Scalar,
) -> Result<Vec<Mesh>> {
    let wall_mm = args
        .thickness_mm
        .and_then(|thickness_mm| plug_wall_mm(args.mode.into(), thickness_mm));
    let gap_mm = args.plug_gap_mm - drift_mm;
    let made = plugs(&mut None, mesh, &Bvh::build(mesh), holes, wall_mm, gap_mm)
        .context("cutting the plugs for the drain holes")?;
    if made.sampled > 0 {
        tracing::warn!(
            "{} of {} plugs follow the surface only approximately: the model could not be read as a solid",
            made.sampled,
            made.meshes.len()
        );
    }
    if made.closed > 0 {
        tracing::warn!(
            "{} of {} holes get no plug: a gap of {gap_mm} mm closes their mouth",
            made.closed,
            made.meshes.len()
        );
    }
    let bounds = mesh.aabb().context("mesh has no vertices")?;
    Ok(laid_beside(
        &made.meshes,
        holes,
        Transform::default(),
        bounds,
    ))
}

/// `holes` as a model hollowed to `wall` cuts them: deepened through the wall, where there
/// is one, so none stops inside it.
pub fn pierced(holes: &[DrainHole], wall: Option<(Scalar, Scalar)>) -> Vec<DrainHole> {
    match wall {
        Some((thickness_mm, voxel_mm)) => pierce(holes, thickness_mm, voxel_mm),
        None => holes.to_vec(),
    }
}

/// The bodies `placed` cuts out of a model hollowed to `wall`, and how many of each.
///
/// They are wound inward, which is what subtracts them wherever they are appended
/// (ADR 0071, 0075), so this needs no cavity: a hole is a hole in a solid model too.
/// `wall` is the cavity the model was hollowed to, and deepens a hole that would
/// otherwise stop inside that wall.
pub fn cut(
    placed: (Vec<DrainHole>, Vec<Channel>),
    wall: Option<(Scalar, Scalar)>,
) -> Result<(Cuts, Mesh)> {
    let (holes, channels) = placed;
    let holes = pierced(&holes, wall);
    let bodies = drill(&holes, &channels).context("drilling the drains")?;
    Ok((
        Cuts {
            holes: holes.len(),
            channels: channels.len(),
        },
        bodies,
    ))
}

/// What the cuts took out of the model.
pub struct Cuts {
    holes: usize,
    channels: usize,
}

impl fmt::Display for Cuts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "  drains        {} hole(s), {} channel(s)",
            self.holes, self.channels
        )
    }
}

/// What the hollowing run did to the model.
pub struct HollowReport {
    settings: HollowSettings,
    faces_before: usize,
    faces_after: usize,
    cavity_mm3: Scalar,
    voxel_mm: Scalar,
    coarsened: bool,
    took: Duration,
}

impl HollowReport {
    /// How much the cavity took out, cubic millimetres.
    pub fn cavity_mm3(&self) -> Scalar {
        self.cavity_mm3
    }

    /// The wall the cavity was cut to and the lattice it was cut on, millimetres: what a
    /// hole has to reach through to drain it.
    pub fn wall(&self) -> (Scalar, Scalar) {
        (self.settings.thickness_mm, self.voxel_mm)
    }

    /// True when the wall left room for a cavity at all.
    pub fn is_clean(&self) -> bool {
        self.cavity_mm3 > 0.0
    }
}

impl fmt::Display for HollowReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "  wall          {:.3} mm, {}{}",
            self.settings.thickness_mm,
            self.settings.mode.label().to_lowercase(),
            if self.settings.open_toward.is_some() {
                ", open through the floor"
            } else {
                ""
            }
        )?;
        writeln!(
            f,
            "  precision     {:.2}, {:.3} mm lattice{}",
            self.settings.precision,
            self.voxel_mm,
            if self.coarsened {
                " (coarsened to fit the memory budget)"
            } else {
                ""
            }
        )?;
        if let Some(infill) = &self.settings.infill {
            let pattern = infill.pattern.label().to_lowercase();
            let part = if infill.pattern.has_walls() {
                "walls"
            } else {
                "struts"
            };
            writeln!(
                f,
                "  infill        {pattern}, {:.1} mm cells at {:.0}%, {:.2} mm {part}",
                infill.cell_mm,
                infill.density * 100.0,
                infill.wall_mm()
            )?;
        }
        writeln!(
            f,
            "  faces         {} -> {}",
            self.faces_before, self.faces_after
        )?;
        writeln!(f, "  resin saved   {:.3} mm^3", self.cavity_mm3)?;
        if !self.is_clean() {
            writeln!(f, "  hollow defect the wall left no room for a cavity")?;
        }
        writeln!(f, "  took          {:.3} s", self.took.as_secs_f64())
    }
}
