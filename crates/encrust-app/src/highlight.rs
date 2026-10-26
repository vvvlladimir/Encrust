//! Marks laid over the models in the viewport, each in a colour of its own, and the places
//! on them the camera can be brought to: whatever a tool points at on a model.

// TODO(step-10): Check's islands marked through this, each a `Spot` the camera flies to.

use std::sync::Arc;

use core_geometry::{Mesh, Region, Transform, Vec3};
use egui::Color32;

/// Millimetres a marked face stands off the surface it marks, so the two never fight for
/// one depth: as far as a painted support patch stands.
pub const LIFT_MM: f32 = 0.05;

/// Faces of a model marked in one colour.
#[derive(Debug, Clone)]
pub struct FaceMark {
    /// The faces as a mesh of their own, in the model's space, stood off its surface.
    pub mesh: Arc<Mesh>,
    pub transform: Transform,
    pub colour: Color32,
}

/// Edges marked in one colour, as their two ends in plate millimetres.
#[derive(Debug, Clone)]
pub struct EdgeMark {
    pub edges: Vec<[Vec3; 2]>,
    pub colour: Color32,
}

/// Everything marked over the models this frame, drawn in the order it is listed.
#[derive(Debug, Clone, Default)]
pub struct Highlight {
    pub faces: Vec<FaceMark>,
    pub edges: Vec<EdgeMark>,
}

/// `faces` of `model` as a mesh of their own, stood `lift_mm` off its surface; `None` for
/// none. A mark over another stands further off, so it wins the depth.
pub fn patch(
    model: &Mesh,
    faces: impl IntoIterator<Item = usize>,
    lift_mm: f32,
) -> Option<Arc<Mesh>> {
    let mut region = Region::default();
    for face in faces {
        region.set(face, true);
    }
    region
        .submesh(model, Transform::default(), lift_mm)
        .map(Arc::new)
}

/// A place on a model the camera can be brought to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spot {
    pub center: Vec3,
    /// How far it reaches from its centre, mm.
    pub radius_mm: f32,
    /// The side it is best seen from, unit length, or zero where it has none.
    pub normal: Vec3,
}

impl Spot {
    /// The spot covering `points`, seen from along `normal`; `None` for no points.
    pub fn around(points: impl IntoIterator<Item = Vec3>, normal: Vec3) -> Option<Self> {
        let points: Vec<Vec3> = points.into_iter().collect();
        let (mins, maxs) = points.iter().fold(
            (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
            |(mins, maxs), point| (mins.min(*point), maxs.max(*point)),
        );
        let center = (mins + maxs) / 2.0;
        let radius_mm = points
            .iter()
            .map(|point| point.distance(center))
            .reduce(f32::max)?;
        Some(Self {
            center,
            radius_mm,
            normal: normal.normalize_or_zero(),
        })
    }

    /// The same spot on a model placed by `transform`, in plate millimetres.
    pub fn placed(&self, transform: Transform) -> Self {
        let matrix = transform.to_matrix();
        let linear = transform.linear();
        let stretch = [linear.x_axis, linear.y_axis, linear.z_axis]
            .map(Vec3::length)
            .into_iter()
            .fold(0.0, f32::max);
        // A normal is carried by the inverse transpose, which keeps it off a stretched face.
        let normal = transform.inverse_matrix().map_or(Vec3::ZERO, |inverse| {
            inverse.transpose().transform_vector3(self.normal)
        });
        Self {
            center: matrix.transform_point3(self.center),
            radius_mm: self.radius_mm * stretch,
            normal: normal.normalize_or_zero(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::Quat;

    #[test]
    fn a_spot_covers_its_points_from_their_middle() {
        let spot = Spot::around([Vec3::ZERO, Vec3::new(2.0, 0.0, 0.0)], Vec3::Z * 5.0)
            .expect("two points");
        assert_eq!(spot.center, Vec3::new(1.0, 0.0, 0.0));
        assert!(
            (spot.radius_mm - 1.0).abs() < 1e-6,
            "got {}",
            spot.radius_mm
        );
        assert_eq!(spot.normal, Vec3::Z, "the normal comes back unit length");
        assert!(Spot::around([], Vec3::Z).is_none());
    }

    #[test]
    fn a_spot_rides_its_model_turned_doubled_and_moved() {
        let spot =
            Spot::around([Vec3::ZERO, Vec3::new(2.0, 0.0, 0.0)], Vec3::Z).expect("two points");
        let transform = Transform {
            translation: Vec3::new(10.0, 0.0, 0.0),
            rotation: Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            scale: Vec3::splat(2.0),
            ..Transform::default()
        };
        let placed = spot.placed(transform);
        assert!((placed.center - Vec3::new(12.0, 0.0, 0.0)).length() < 1e-5);
        assert!(
            (placed.radius_mm - 2.0).abs() < 1e-5,
            "doubled: {}",
            placed.radius_mm
        );
        assert!(
            (placed.normal - Vec3::new(0.0, -1.0, 0.0)).length() < 1e-5,
            "+Z turned a quarter about X faces -Y; got {}",
            placed.normal
        );
    }
}
