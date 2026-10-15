//! The poses a model is measured in: those a person tries by hand, what each goal chooses,
//! and, asked for, an even spread over the sphere that stands in for the true best.

use std::time::Instant;

use anyhow::{Context, Result};
use core_engine::{Printout, Trial, Tried, print_out, prove};
use core_geometry::{Mesh, Quat, Vec3};
use core_plate::{
    OrientGoal, OrientSettings, Oriented, Printed, Ruling, finalists, flat_faces, rotation_for,
};
use encrust_cli::pipeline;
use printer_profiles::{BuildVolume, SupportProfile};
use serde::{Deserialize, Serialize};

/// Where a pose came from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Source {
    Hand,
    Goal(OrientGoal),
    Oracle,
    /// A pose a goal chose in a run kept before, measured again so that what is known to
    /// be possible is not forgotten when the search stops finding it.
    Known,
}

/// A pose to measure again: its name, its turn and whether it stands on the plate.
pub type Known = (String, Quat, bool);

/// One way of standing a model and what it prints.
#[derive(Debug, Clone)]
pub struct Pose {
    pub name: String,
    pub source: Source,
    pub rotation: Quat,
    pub on_plate: bool,
    pub printout: Printout,
    /// How far the model's own up leans from the plate's, degrees: over 90 is upside down.
    pub tilt_deg: f32,
    /// Whether the pose, supports and all, fits the build volume, turned about the
    /// vertical however it fits best.
    pub fits: bool,
    /// Whether a face lies flat on the plate, so that set there the model sits rather
    /// than balances on an edge or a point.
    pub sits: bool,
    pub seconds: f64,
    /// For a goal's choice, how it was made; empty for any other pose.
    pub trace: Trace,
}

/// How a goal chose: the seconds each stage took, and every finalist it printed.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Trace {
    /// Finding the finalists off the mesh: scoring every candidate and cutting the shortlist.
    pub search_s: f64,
    /// Printing the finalists, coarse, and ranking them.
    pub proof_s: f64,
    /// Best first, as the goal ranked them.
    pub finalists: Vec<Finalist>,
}

/// One finalist as the proof read it on its coarse stack, and what decided its place.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Finalist {
    pub tilt_deg: f32,
    pub on_plate: bool,
    pub fatal: usize,
    /// Islands and hanging contacts.
    pub unheld: usize,
    /// The goal's own reading.
    pub reading: f64,
    pub ruling: String,
    /// The same pose printed at full resolution, where the run asked for it.
    #[serde(default)]
    pub full: Option<Full>,
    /// Where the search ranked it among its finalists, best first.
    #[serde(default)]
    pub searched: usize,
}

/// A finalist's readings printed at full resolution, to set against the coarse ones.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Full {
    pub fatal: usize,
    pub unheld: usize,
    pub reading: f64,
}

impl Full {
    fn of(printout: &Printout, goal: OrientGoal) -> Self {
        Self {
            fatal: printout.fatal(),
            unheld: printout.islands + printout.hanging,
            reading: printout.reading(goal),
        }
    }
}

impl Finalist {
    fn of(tried: &Tried, goal: OrientGoal) -> Self {
        let printout = &tried.proven.printout;
        Self {
            tilt_deg: tilt_deg(tried.proven.oriented.rotation),
            on_plate: tried.proven.oriented.on_plate,
            fatal: printout.fatal(),
            unheld: printout.islands + printout.hanging,
            reading: printout.reading(goal),
            ruling: ruling(tried.ruling).to_owned(),
            full: None,
            searched: 0,
        }
    }
}

fn ruling(ruling: Ruling) -> &'static str {
    match ruling {
        Ruling::Chosen => "chosen",
        Ruling::Stayed => "stayed as it stands",
        Ruling::NotWorthTurning => "not worth the turn",
        Ruling::Fatal => "fails the print",
        Ruling::Unheld => "leaves too much hanging",
        Ruling::OutOfBand => "out of the goal's band",
        Ruling::TieBreak => "lost the tie-break",
    }
}

/// What every pose is measured against.
pub struct Bench<'a> {
    pub trial: Trial<'a>,
    pub support: &'a SupportProfile,
    pub volume: &'a BuildVolume,
    /// The goal whose finalists are printed again at full resolution, beside the proof.
    pub full_finalists: Option<OrientGoal>,
}

impl Bench<'_> {
    /// Every pose of `mesh`: by hand, each goal's choice, `known` from runs before, then
    /// `oracle` directions spread evenly over the sphere.
    pub fn poses(&self, mesh: &Mesh, oracle: usize, known: &[Known]) -> Result<Vec<Pose>> {
        let printed = Printed::solid(mesh);
        let mut poses = Vec::new();
        for (name, rotation, on_plate) in by_hand(mesh) {
            poses.push(self.measure(&printed, name, Source::Hand, rotation, on_plate)?);
        }
        for goal in OrientGoal::ALL {
            poses.push(self.chosen(&printed, goal)?);
        }
        for (name, rotation, on_plate) in known {
            let measured = |pose: &Pose| {
                pose.on_plate == *on_plate && pose.rotation.angle_between(*rotation) < KNOWN_DEG
            };
            if !poses.iter().any(measured) {
                let name = format!("known: {name}");
                poses.push(self.measure(&printed, name, Source::Known, *rotation, *on_plate)?);
            }
        }
        for (at, down) in spread(oracle).into_iter().enumerate() {
            let name = format!("oracle {at}");
            let rotation = rotation_for(down);
            poses.push(self.measure(&printed, name, Source::Oracle, rotation, false)?);
        }
        Ok(poses)
    }

    /// What `goal` chooses, the way the window asks it, judged on the stack it prints at
    /// full resolution: the proof itself ranks on a coarse one.
    fn chosen(&self, printed: &Printed, goal: OrientGoal) -> Result<Pose> {
        let settings = OrientSettings {
            goal,
            ..pipeline::orient_settings(Some(self.support))
        };
        let started = Instant::now();
        let found = finalists(printed, &settings, usize::MAX)?;
        let search_s = started.elapsed().as_secs_f64();
        let tried = prove(printed, &settings, &found, &self.trial)?;
        let seconds = started.elapsed().as_secs_f64();
        let chosen = tried
            .first()
            .context("the search found no orientation")?
            .proven
            .oriented;
        let name = format!("goal: {}", goal.label());
        let mut pose = self.measure(
            printed,
            name,
            Source::Goal(goal),
            chosen.rotation,
            chosen.on_plate,
        )?;
        pose.seconds = seconds;
        pose.trace = Trace {
            search_s,
            proof_s: seconds - search_s,
            finalists: tried
                .iter()
                .map(|tried| self.finalist(printed, tried, goal, &found))
                .collect::<Result<_>>()?,
        };
        Ok(pose)
    }

    fn finalist(
        &self,
        printed: &Printed,
        tried: &Tried,
        goal: OrientGoal,
        found: &[Oriented],
    ) -> Result<Finalist> {
        let mut finalist = Finalist::of(tried, goal);
        finalist.searched = found
            .iter()
            .position(|pose| {
                pose.on_plate == tried.proven.oriented.on_plate
                    && pose.rotation == tried.proven.oriented.rotation
            })
            .unwrap_or(usize::MAX);
        if self.full_finalists == Some(goal) {
            let oriented = tried.proven.oriented;
            let printout = print_out(printed, oriented.rotation, oriented.on_plate, &self.trial)?;
            finalist.full = Some(Full::of(&printout, goal));
        }
        Ok(finalist)
    }

    fn measure(
        &self,
        printed: &Printed,
        name: impl Into<String>,
        source: Source,
        rotation: Quat,
        on_plate: bool,
    ) -> Result<Pose> {
        let started = Instant::now();
        let printout = print_out(printed, rotation, on_plate, &self.trial)?;
        let lift_mm = if on_plate {
            0.0
        } else {
            self.support.lift_mm()
        };
        Ok(Pose {
            name: name.into(),
            source,
            rotation,
            on_plate,
            printout,
            tilt_deg: tilt_deg(rotation),
            fits: fits(printed.outer, rotation, lift_mm, self.volume),
            sits: sits(printed.outer, rotation),
            seconds: started.elapsed().as_secs_f64(),
            trace: Trace::default(),
        })
    }
}

/// A known pose this near one already measured is the same pose, radians.
const KNOWN_DEG: f32 = 0.5 * std::f32::consts::PI / 180.0;

/// The poses a person tries first: as drawn, on supports and on the plate, the largest
/// flat face down, and tilted onto a corner.
fn by_hand(mesh: &Mesh) -> [(&'static str, Quat, bool); 4] {
    let largest = flat_faces(mesh)
        .first()
        .map_or(Vec3::NEG_Z, |flat| flat.normal);
    let corner = Vec3::new(1.0, 1.0, 0.0).normalize();
    [
        ("as is", Quat::IDENTITY, false),
        ("as is, on the plate", Quat::IDENTITY, true),
        ("largest face down", rotation_for(largest), false),
        (
            "tilted 35° on a corner",
            Quat::from_axis_angle(corner, 35f32.to_radians()),
            false,
        ),
    ]
}

/// How far `rotation` leans the model's own up away from the plate's, degrees.
pub fn tilt_deg(rotation: Quat) -> f32 {
    (rotation * Vec3::Z).z.clamp(-1.0, 1.0).acos().to_degrees()
}

/// `count` directions spread evenly over the sphere, by the golden angle.
fn spread(count: usize) -> Vec<Vec3> {
    let golden = std::f32::consts::PI * (3.0 - 5f32.sqrt());
    (0..count)
        .map(|at| {
            let z = 1.0 - 2.0 * (at as f32 + 0.5) / count as f32;
            let ring = (1.0 - z * z).max(0.0).sqrt();
            let around = golden * at as f32;
            Vec3::new(ring * around.cos(), ring * around.sin(), z)
        })
        .collect()
}

/// A face this near straight down and this near the lowest point lies on the plate, as
/// the search reads a footprint.
const FLAT: f32 = 0.996;
const SEATED_MM: f32 = 0.5;
/// The least footprint a model sits on rather than balances, square millimetres.
const SITS_MM2: f32 = 1.0;

/// Whether `mesh` turned by `rotation` has a footprint flat on the plate.
fn sits(mesh: &Mesh, rotation: Quat) -> bool {
    let low = mesh
        .vertices
        .iter()
        .map(|vertex| (rotation * *vertex).z)
        .fold(f32::INFINITY, f32::min);
    let footprint_mm2: f32 = mesh
        .triangles()
        .filter_map(|triangle| {
            let [a, b, c] = [triangle.a, triangle.b, triangle.c].map(|v| rotation * v);
            let normal = (b - a).cross(c - a);
            let area = normal.length() / 2.0;
            let low_enough = a.z.max(b.z).max(c.z) - low < SEATED_MM;
            (area > 0.0 && normal.z / (2.0 * area) < -FLAT && low_enough).then_some(area)
        })
        .sum();
    footprint_mm2 >= SITS_MM2
}

/// Whether `mesh` turned by `rotation` and stood `lift_mm` off the plate fits `volume`.
/// The plate is free to turn it about the vertical, so its two widths are matched with the
/// plate's sorted.
fn fits(mesh: &Mesh, rotation: Quat, lift_mm: f32, volume: &BuildVolume) -> bool {
    let (mins, maxs) = mesh.vertices.iter().fold(
        (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
        |(mins, maxs), vertex| {
            let turned = rotation * *vertex;
            (mins.min(turned), maxs.max(turned))
        },
    );
    let size = maxs - mins;
    let (narrow, wide) = (size.x.min(size.y), size.x.max(size.y));
    let (plate_narrow, plate_wide) = (volume.x.min(volume.y), volume.x.max(volume.y));
    narrow <= plate_narrow && wide <= plate_wide && size.z + lift_mm <= volume.z
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_model_turned_over_leans_its_up_by_half_a_turn() {
        let over = Quat::from_rotation_x(std::f32::consts::PI);
        assert!((tilt_deg(over) - 180.0).abs() < 1e-3);
        assert!(tilt_deg(Quat::IDENTITY).abs() < 1e-3);
    }

    #[test]
    fn the_spread_is_unit_directions_covering_both_poles() {
        let directions = spread(64);
        assert!(directions.iter().all(|d| (d.length() - 1.0).abs() < 1e-4));
        assert!(directions.iter().any(|d| d.z > 0.9));
        assert!(directions.iter().any(|d| d.z < -0.9));
    }

    #[test]
    fn a_long_part_fits_only_lying_along_the_wide_side_of_the_plate() {
        let rod = Mesh::new(
            vec![
                Vec3::ZERO,
                Vec3::new(150.0, 0.0, 0.0),
                Vec3::new(0.0, 10.0, 10.0),
            ],
            vec![[0, 1, 2]],
        );
        let volume = BuildVolume {
            x: 120.0,
            y: 200.0,
            z: 100.0,
        };
        assert!(
            fits(&rod, Quat::IDENTITY, 0.0, &volume),
            "150 mm lies along y"
        );
        let upright = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        assert!(
            !fits(&rod, upright, 0.0, &volume),
            "150 mm stood up is over 100 mm"
        );
    }
}
