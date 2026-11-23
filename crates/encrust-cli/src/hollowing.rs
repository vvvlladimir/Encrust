use std::fmt;
use std::time::Duration;

use anyhow::{Result, bail};
use core_geometry::{Scalar, Vec3};
use core_volume::{
    CHANNEL_DIAMETER_MM, HoleShape, HoleSize, HollowMode, HollowSettings, InfillPattern,
    InfillSettings, PLUG_GAP_MM, plug_wall_mm,
};

use crate::keep_solid::KeepSolidArgs;

mod asked;
mod auto;

pub use auto::AutoReport;

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
    #[arg(long = "drain", value_name = "MM", default_value_t = HoleSize::default().diameter_mm)]
    pub drain_mm: Scalar,

    /// Drill a drain hole into the surface nearest this point, plate mm from the front left corner
    /// as the model prints: after placement and the lift supports give it, the frame the report
    /// locates findings in. Repeat for more holes. `auto` drills the holes the engine proves
    /// drain every pocket still trapped.
    #[arg(long = "drain-at", value_name = "X,Y,Z|auto", value_parser = drain_at, allow_hyphen_values = true)]
    pub drain_at: Vec<DrainAt>,

    /// Leave out the vent `--drain-at auto` drills high on each pocket it drains.
    #[arg(long = "no-drain-vent")]
    pub no_vent: bool,

    /// How far past the surface a drain hole reaches, millimetres.
    #[arg(long = "drain-depth", value_name = "MM", default_value_t = HoleSize::default().depth_mm)]
    pub drain_depth_mm: Scalar,

    /// Diameter of a hole's far end as a fraction of its mouth: 1 is a cylinder.
    #[arg(long = "drain-taper", value_name = "0..1", default_value_t = HoleSize::default().taper)]
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

    /// Drill the drain holes into the model before it is hollowed: the cavity keeps a wall
    /// around each and it keeps its depth, a recess in the shape rather than a drain.
    #[arg(long = "drain-walled")]
    pub walled: bool,

    /// Drill a hole into the raft nearest this point, plate mm, as it stands once built, sized
    /// as --drain asks; needs supports on a raft. Repeat for more holes.
    #[arg(long = "drain-support-at", value_name = "X,Y,Z", value_parser = point, allow_hyphen_values = true)]
    pub drain_support_at: Vec<Vec3>,

    /// Print a plug for each drain hole, standing on the plate beside the model.
    #[arg(long = "drain-plugs")]
    pub plugs: bool,

    /// How much narrower than its hole a plug is drawn, millimetres a side. A resin whose
    /// tolerance compensation is off takes its hole and outer offsets off this too.
    #[arg(long = "plug-gap", value_name = "MM", default_value_t = PLUG_GAP_MM)]
    pub plug_gap_mm: Scalar,

    /// Dig a drainage channel along a polyline of points, `x,y,z:x,y,z:...`, in the same
    /// plate millimetres as --drain-at, lift included. Repeat for more than one channel.
    #[arg(long = "channel", value_name = "X,Y,Z:...", value_parser = polyline, allow_hyphen_values = true)]
    pub channels: Vec<Vec<Vec3>>,

    /// Diameter of the drainage channels, millimetres.
    #[arg(long = "channel-size", value_name = "MM", default_value_t = CHANNEL_DIAMETER_MM)]
    pub channel_mm: Scalar,

    #[command(flatten)]
    pub keep_solid: KeepSolidArgs,
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

/// Where `--drain-at` drills: by a point, or where the engine proves a hole drains.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DrainAt {
    Point(Vec3),
    Auto,
}

fn drain_at(text: &str) -> Result<DrainAt, String> {
    match text.trim() {
        "auto" => Ok(DrainAt::Auto),
        _ => point(text).map(DrainAt::Point),
    }
}

/// A polyline of points separated by colons.
pub(crate) fn polyline(text: &str) -> Result<Vec<Vec3>, String> {
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

    /// Whether a cut was asked for at a point, which means one model.
    pub fn pointing(&self) -> bool {
        self.points().next().is_some() || !self.channels.is_empty()
    }

    /// Whether `--drain-at auto` was given.
    pub fn auto(&self) -> bool {
        self.drain_at.contains(&DrainAt::Auto)
    }

    /// The points `--drain-at` drills at.
    pub(crate) fn points(&self) -> impl Iterator<Item = Vec3> + '_ {
        self.drain_at.iter().filter_map(|at| match at {
            DrainAt::Point(point) => Some(*point),
            DrainAt::Auto => None,
        })
    }

    /// Refuses an infill or an open floor on anything but a shell, and plugs with no hole to
    /// close or a gap that would make them wider than it.
    pub(crate) fn check_flags(&self) -> Result<()> {
        self.check_infill()?;
        self.keep_solid.check_flags(self.wanted())?;
        if self.infill.is_some() && !self.wanted() {
            bail!("--infill fills a shell's cavity, and --hollow was not given");
        }
        if self.open_floor && !self.wanted() {
            bail!("--hollow-open-floor opens a shell's floor, and --hollow was not given");
        }
        if self.open_floor && matches!(self.mode, Mode::External) {
            bail!("--hollow-open-floor opens a shell's floor, and a mould has none");
        }
        if self.plugs && self.drain_at.is_empty() {
            bail!("--drain-plugs closes the holes --drain-at drills, and none was given");
        }
        if self.no_vent && !self.auto() {
            bail!(
                "--no-drain-vent leaves out the vent --drain-at auto drills, and it was not given"
            );
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
    pub(crate) fn hole_size(&self) -> HoleSize {
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

    /// How long a plug for a hole through this wall is, millimetres: `plug_wall_mm`'s.
    pub fn plug_wall_mm(&self) -> Option<Scalar> {
        plug_wall_mm(self.settings.mode, self.settings.thickness_mm)
    }

    /// Whether the shell begins with the model byte for byte: a mould and an opened floor do not.
    pub fn on_its_model(&self) -> bool {
        self.settings.mode == HollowMode::Internal && self.settings.open_toward.is_none()
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
