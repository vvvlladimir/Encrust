use std::fmt;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use core_geometry::{Bvh, Mesh, Scalar, Vec3};
use core_volume::{
    Channel, DrainHole, HollowMode, HollowSettings, InfillPattern, InfillSettings, drill, hole_at,
    hollow, pierce, sleeves,
};

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

    /// Fill the cavity with a lattice.
    #[arg(long = "infill", value_name = "PATTERN")]
    pub infill: Option<Pattern>,

    /// Side of one infill cell, millimetres. For a hive, a hexagon's diameter.
    #[arg(long = "infill-size", value_name = "MM", default_value_t = 5.0)]
    pub size_mm: Scalar,

    /// How much of the cavity the lattice fills, 0 to 1.
    #[arg(long = "infill-density", value_name = "0..1", default_value_t = 0.15)]
    pub density: Scalar,

    /// Diameter of the drain holes, millimetres.
    #[arg(long = "drain", value_name = "MM", default_value_t = 3.0)]
    pub drain_mm: Scalar,

    /// Drill a drain hole into the surface nearest this point, in plate millimetres after
    /// any placement. Repeat for more than one hole.
    #[arg(long = "drain-at", value_name = "X,Y,Z", value_parser = point)]
    pub drain_at: Vec<Vec3>,

    /// How far past the surface a drain hole reaches, millimetres.
    #[arg(long = "drain-depth", value_name = "MM", default_value_t = 3.0)]
    pub drain_depth_mm: Scalar,

    /// Diameter of a hole's far end as a fraction of its mouth: 1 is a cylinder.
    #[arg(long = "drain-taper", value_name = "0..1", default_value_t = 1.0)]
    pub drain_taper: Scalar,

    /// Dig a drainage channel along a polyline of points, `x,y,z:x,y,z:...`, in plate
    /// millimetres. Repeat for more than one channel.
    #[arg(long = "channel", value_name = "X,Y,Z:...", value_parser = polyline)]
    pub channels: Vec<Vec<Vec3>>,

    /// Diameter of the drainage channels, millimetres.
    #[arg(long = "channel-size", value_name = "MM", default_value_t = 2.0)]
    pub channel_mm: Scalar,
}

/// One `x,y,z` point of the command line, in plate millimetres.
fn point(text: &str) -> Result<Vec3, String> {
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
            infill: self.infill.map(|pattern| InfillSettings {
                pattern: pattern.into(),
                size_mm: self.size_mm,
                density: self.density,
            }),
            blockers: sleeves(&self.dug(), thickness_mm),
            ..HollowSettings::default()
        })
    }

    fn dug(&self) -> Vec<Channel> {
        self.channels
            .iter()
            .map(|points| Channel {
                points: points.clone(),
                diameter_mm: self.channel_mm,
            })
            .collect()
    }

    /// A hole on the surface nearest each `--drain-at`, drilled straight into it.
    fn drains(&self, mesh: &Mesh, bvh: &Bvh) -> Result<Vec<DrainHole>> {
        self.drain_at
            .iter()
            .map(|near| {
                hole_at(
                    mesh,
                    bvh,
                    *near,
                    self.drain_mm,
                    self.drain_depth_mm,
                    self.drain_taper,
                )
                .with_context(|| format!("no surface near {near} to drill a drain hole into"))
            })
            .collect()
    }
}

/// Hollows the mesh and says what that cost and what it saved.
pub fn run(mesh: &Mesh, args: &HollowArgs, precision: Scalar) -> Result<(Mesh, HollowReport)> {
    let started = Instant::now();
    let bvh = Bvh::build(mesh);
    let Some(settings) = args.settings(precision) else {
        bail!("--hollow was not given, so there is nothing to hollow");
    };
    let hollowed = hollow(mesh, &bvh, &settings).context("hollowing the model")?;

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

/// Cuts `placed` into `mesh`, and says how many of each it took out.
///
/// The bodies are appended wound inward, which is what subtracts them (ADR 0071, 0075), so
/// this needs no cavity: a hole is a hole in a solid model too. `wall` is the cavity the
/// model was hollowed to, and deepens a hole that would otherwise stop inside that wall.
pub fn cut(
    mesh: &mut Mesh,
    placed: (Vec<DrainHole>, Vec<Channel>),
    wall: Option<(Scalar, Scalar)>,
) -> Result<Cuts> {
    let (holes, channels) = placed;
    let holes = match wall {
        Some((thickness_mm, voxel_mm)) => pierce(&holes, thickness_mm, voxel_mm),
        None => holes,
    };
    let bodies = drill(&holes, &channels).context("drilling the drains")?;

    let offset = mesh.vertices.len() as u32;
    mesh.vertices.extend_from_slice(&bodies.vertices);
    mesh.faces.extend(
        bodies
            .faces
            .iter()
            .map(|[a, b, c]| [a + offset, b + offset, c + offset]),
    );
    Ok(Cuts {
        holes: holes.len(),
        channels: channels.len(),
    })
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
            "  wall          {:.3} mm, {}",
            self.settings.thickness_mm,
            self.settings.mode.label().to_lowercase()
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
            writeln!(
                f,
                "  infill        {}, {:.1} mm cells at {:.0}%, {:.2} mm walls",
                infill.pattern.label().to_lowercase(),
                infill.size_mm,
                infill.density * 100.0,
                infill.thickness_mm()
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
