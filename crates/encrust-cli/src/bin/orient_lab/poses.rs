//! The poses a model is measured in: those a person tries by hand, what each goal chooses,
//! and, asked for, an even spread over the sphere that stands in for the true best.

use std::time::Instant;

use anyhow::{Context, Result};
use core_engine::{Printout, Trial, orient_proven, print_out};
use core_geometry::{Mesh, Quat, Vec3};
use core_plate::{OrientGoal, OrientSettings, Printed, flat_faces, rotation_for};
use encrust_cli::pipeline;
use printer_profiles::{BuildVolume, SupportProfile};

/// Where a pose came from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Source {
    Hand,
    Goal(OrientGoal),
    Oracle,
}

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
    pub seconds: f64,
}

/// What every pose is measured against.
pub struct Bench<'a> {
    pub trial: Trial<'a>,
    pub support: &'a SupportProfile,
    pub volume: &'a BuildVolume,
}

impl Bench<'_> {
    /// Every pose of `mesh`: by hand, each goal's choice, then `oracle` directions spread
    /// evenly over the sphere.
    pub fn poses(&self, mesh: &Mesh, oracle: usize) -> Result<Vec<Pose>> {
        let printed = Printed::solid(mesh);
        let mut poses = Vec::new();
        for (name, rotation, on_plate) in by_hand(mesh) {
            poses.push(self.measure(&printed, name, Source::Hand, rotation, on_plate)?);
        }
        for goal in OrientGoal::ALL {
            poses.push(self.chosen(&printed, goal)?);
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
        let found = orient_proven(printed, &settings, &self.trial)
            .context("the search found no orientation")?
            .oriented;
        let seconds = started.elapsed().as_secs_f64();
        let name = format!("goal: {}", goal.label());
        let mut pose = self.measure(
            printed,
            name,
            Source::Goal(goal),
            found.rotation,
            found.on_plate,
        )?;
        pose.seconds = seconds;
        Ok(pose)
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
            seconds: started.elapsed().as_secs_f64(),
        })
    }
}

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
