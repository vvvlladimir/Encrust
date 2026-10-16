use core_geometry::{Mesh, Scalar, Vec2, Vec3, fitting_turn};
use rayon::prelude::*;

use crate::candidates::sphere;
use crate::orient::rotation_for;

/// Directions the envelope takes the model's farthest vertex along, each one of its
/// vertices.
const ENVELOPE_DIRECTIONS: usize = 256;
/// How far inside the hull the envelope may fall, as a share of the model's radius: 1 - cos
/// of the 0.13 rad no direction lies farther than from one of the 256.
const ENVELOPE_SHORT: Scalar = 0.01;
/// How far the foot of supports reaches past the shadow of what they hold on every side,
/// millimetres: a raft's own margin (ADR 0241).
const BASE_MARGIN_MM: Scalar = 2.0;

/// The model's convex hull cut down to a few vertices, what a pose is checked against the
/// build volume by: `docs/design/orientation.md`.
pub struct Envelope {
    points: Vec<Vec3>,
    /// How far each extent read off the points may fall short, millimetres.
    short_mm: Scalar,
    room_mm: Vec3,
}

impl Envelope {
    /// The envelope of `mesh` in its own space, checked against a build volume `room_mm`.
    pub fn of(mesh: &Mesh, room_mm: Vec3) -> Self {
        let mut points: Vec<Vec3> = sphere(ENVELOPE_DIRECTIONS)
            .into_par_iter()
            .filter_map(|along| {
                let reach = |vertex: &&Vec3| vertex.dot(along);
                mesh.vertices
                    .iter()
                    .max_by(|a, b| reach(a).total_cmp(&reach(b)))
                    .copied()
            })
            .collect();
        points.sort_by(|a, b| {
            (a.x.total_cmp(&b.x))
                .then(a.y.total_cmp(&b.y))
                .then(a.z.total_cmp(&b.z))
        });
        points.dedup();
        let radius_mm = mesh
            .aabb()
            .map_or(0.0, |bounds| (bounds.maxs - bounds.mins).length() / 2.0);
        Self {
            points,
            short_mm: ENVELOPE_SHORT * radius_mm,
            room_mm,
        }
    }

    /// The turn about the vertical, radians, that stands the model inside the build volume
    /// with `down` at the plate and lifted `lift_mm` onto supports, or `None` where none does.
    pub fn turn(&self, down: Vec3, lift_mm: Scalar) -> Option<Scalar> {
        let rotation = rotation_for(down);
        let turned: Vec<Vec3> = self.points.iter().map(|point| rotation * *point).collect();
        let (low, high) = turned.iter().fold(
            (Scalar::INFINITY, Scalar::NEG_INFINITY),
            |(low, high), point| (low.min(point.z), high.max(point.z)),
        );
        let height_mm = high - low + 2.0 * self.short_mm + lift_mm;
        if height_mm > self.room_mm.z {
            return None;
        }
        let base_mm = if lift_mm > 0.0 { BASE_MARGIN_MM } else { 0.0 };
        let around_mm = 2.0 * (self.short_mm + base_mm);
        let plan: Vec<Vec2> = turned.iter().map(|point| point.truncate()).collect();
        let room = self.room_mm.truncate() - Vec2::splat(around_mm);
        fitting_turn(&plan, room)
    }
}

/// The turn that fits `down` into the build volume `envelope` was made for, and no turn
/// at all where there is no volume to check against.
pub fn turn_to_fit(envelope: Option<&Envelope>, down: Vec3, lift_mm: Scalar) -> Option<Scalar> {
    envelope.map_or(Some(0.0), |envelope| envelope.turn(down, lift_mm))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Axis-aligned box spanning 0..size on every axis.
    fn cuboid(size: Vec3) -> Mesh {
        let vertices = (0..8)
            .map(|at| {
                Vec3::new(
                    size.x * (at & 1) as Scalar,
                    size.y * ((at >> 1) & 1) as Scalar,
                    size.z * (at >> 2) as Scalar,
                )
            })
            .collect();
        Mesh::new(vertices, vec![[0, 1, 2]])
    }

    #[test]
    fn a_box_on_the_plate_fits_a_room_its_own_size_and_a_hair() {
        let room = Vec3::new(40.5, 20.5, 10.5);
        let envelope = Envelope::of(&cuboid(Vec3::new(40.0, 20.0, 10.0)), room);
        // Every corner of a box is the farthest vertex along some direction: the envelope
        // is the box, short by at most a hundredth of its 23 mm radius on each side.
        assert_eq!(envelope.points.len(), 8);
        assert_eq!(envelope.turn(Vec3::NEG_Z, 0.0), Some(0.0));
    }

    #[test]
    fn supports_lift_a_pose_out_of_a_room_it_fits_on_the_plate() {
        let room = Vec3::new(60.0, 30.0, 15.0);
        let envelope = Envelope::of(&cuboid(Vec3::new(40.0, 20.0, 10.0)), room);
        assert!(envelope.turn(Vec3::NEG_Z, 0.0).is_some());
        assert_eq!(
            envelope.turn(Vec3::NEG_Z, 5.0),
            None,
            "10 + 5 mm and the slack"
        );
    }

    #[test]
    fn a_long_bar_fits_across_a_plate_shorter_than_it() {
        // 100 mm long, the bar is longer than the 90 mm plate is wide, but at 45 degrees
        // it spans (100 + 4) / sqrt 2 = 74 mm each way.
        let room = Vec3::new(90.0, 90.0, 50.0);
        let envelope = Envelope::of(&cuboid(Vec3::new(100.0, 4.0, 4.0)), room);
        let turn = envelope
            .turn(Vec3::NEG_Z, 5.0)
            .expect("the bar fits turned");
        assert!(turn.abs() > 0.1, "it had to turn, got {turn}");
    }
}
