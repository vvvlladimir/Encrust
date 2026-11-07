use core_geometry::Transform;

use super::{DrainHole, turn_in_model};

impl DrainHole {
    /// This hole, placed on the plate, in the own space of a model `transform` stands; a hole
    /// is measured on the plate, so its sizes come back through the model's least scale.
    pub fn into_model(self, transform: Transform) -> Option<Self> {
        let inverse = transform.inverse_matrix()?;
        let axis = inverse.transform_vector3(self.axis).try_normalize()?;
        let smallest = transform.least_scale();
        Some(Self {
            at: inverse.transform_point3(self.at),
            axis,
            diameter_mm: self.diameter_mm / smallest,
            depth_mm: self.depth_mm / smallest,
            lift_mm: self.lift_mm / smallest,
            turn_deg: turn_in_model(axis, self.turn_deg, transform.linear()),
            ..self
        })
    }
}

#[cfg(test)]
mod tests {
    use core_geometry::{Quat, Vec3};

    use crate::HoleSize;

    use super::*;

    #[test]
    fn a_hole_on_a_scaled_turned_cube_comes_back_onto_its_face_at_its_own_size() {
        let transform = Transform {
            translation: Vec3::new(10.0, -4.0, 7.0),
            rotation: Quat::from_rotation_z(0.6),
            scale: Vec3::splat(2.0),
            ..Transform::default()
        };
        let matrix = transform.to_matrix();
        // The middle of a 20 mm cube's own +X face, drilled along -X, as the plate sees it.
        let own_at = Vec3::new(20.0, 10.0, 10.0);
        let found = DrainHole::sized(
            matrix.transform_point3(own_at),
            matrix.transform_vector3(Vec3::NEG_X).normalize(),
            HoleSize::round(4.0, 6.0, 1.0),
            0.5,
        );

        let hole = found.into_model(transform).expect("the placement inverts");
        assert!(
            hole.at.distance(own_at) < 1e-3,
            "back at {own_at}, got {}",
            hole.at
        );
        assert!(
            hole.axis.distance(Vec3::NEG_X) < 1e-5,
            "drilled along its own -X, got {}",
            hole.axis
        );
        assert!(
            (hole.diameter_mm - 2.0).abs() < 1e-5 && (hole.depth_mm - 3.0).abs() < 1e-5,
            "4 mm wide and 6 mm deep on the plate is 2 by 3 at twice the size, got {} by {}",
            hole.diameter_mm,
            hole.depth_mm
        );
        assert!(
            (hole.lift_mm - 0.25).abs() < 1e-5,
            "a 0.5 mm lift is 0.25 mm, got {}",
            hole.lift_mm
        );
    }

    #[test]
    fn a_placement_flat_to_nothing_has_no_own_space() {
        let flat = Transform {
            scale: Vec3::new(1.0, 1.0, 0.0),
            ..Transform::default()
        };
        let hole = DrainHole::sized(Vec3::ZERO, Vec3::Z, HoleSize::round(3.0, 3.0, 1.0), 0.0);
        assert_eq!(hole.into_model(flat), None);
    }
}
