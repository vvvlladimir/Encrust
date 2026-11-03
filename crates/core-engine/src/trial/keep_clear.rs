//! Places on a model a person asks no support to touch, as a lip or a face, and the poses
//! ranked to leave them clear (ADR 0403).

use core_geometry::{
    Mesh, Quat, Scalar, Transform, Vec2, Vec3, center_over_plate, drop_to_plate, transform_mesh,
};

use super::Tried;

/// Points on a model no support should touch, in the frame of the mesh the search is given.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct KeepClear {
    pub at_mm: Vec<Vec3>,
    /// How far from a point a contact still touches it, millimetres.
    pub radius_mm: Scalar,
}

impl KeepClear {
    /// Points given on the plate as `mesh` stood turned by `seen`, centred over a plate
    /// `plate_mm` across and on z = 0, taken back into `mesh`'s own frame.
    pub fn seen_turned(
        mesh: &Mesh,
        seen: Quat,
        plate_mm: Vec2,
        at_mm: &[Vec3],
        radius_mm: Scalar,
    ) -> Option<Self> {
        let turn = Transform {
            rotation: seen,
            ..Transform::default()
        };
        let bounds = transform_mesh(mesh, turn).aabb()?;
        let offset = drop_to_plate(&bounds) + center_over_plate(&bounds, plate_mm.x, plate_mm.y);
        let back = seen.inverse();
        Some(Self {
            at_mm: at_mm.iter().map(|at| back * (*at - offset)).collect(),
            radius_mm,
        })
    }

    /// How many `contacts` touch a point, the model turned by `rotation` and moved by `offset`
    /// onto the plate the contacts are on.
    pub(super) fn touched(&self, contacts: &[Vec3], rotation: Quat, offset: Vec3) -> usize {
        let points: Vec<Vec3> = self
            .at_mm
            .iter()
            .map(|at| rotation * *at + offset)
            .collect();
        let reach = self.radius_mm * self.radius_mm;
        contacts
            .iter()
            .filter(|contact| {
                points
                    .iter()
                    .any(|at| at.distance_squared(**contact) <= reach)
            })
            .count()
    }
}

/// `ranked` with the sound poses touching the fewest kept-clear places first, the order it
/// came in kept between equals: a place asked clear outweighs every goal, never a lost print.
pub(super) fn clear_first(ranked: &mut [Tried]) {
    ranked.sort_by_key(|tried| {
        let printout = &tried.proven.printout;
        (
            printout.fatal() > 0 || printout.overflows,
            printout.kept_clear_touched,
        )
    });
}

#[cfg(test)]
mod tests {
    use core_geometry::cuboid;

    use super::*;

    #[test]
    fn a_point_seen_on_a_turned_cube_comes_back_to_the_cube_s_own_corner() {
        // Turned 90 degrees about X, (x, y, z) goes to (x, -z, y): the 10 mm cube spans
        // y -10..0, and centred on a 100 mm plate its corner (10, 0, 10) stands at (55, 45, 0).
        let cube = cuboid(Vec3::ZERO, Vec3::splat(10.0));
        let seen = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        let given = [Vec3::new(55.0, 45.0, 0.0)];
        let kept = KeepClear::seen_turned(&cube, seen, Vec2::splat(100.0), &given, 1.0)
            .expect("a cube has bounds");
        let back = kept.at_mm[0];
        assert!(
            back.distance(Vec3::new(10.0, 0.0, 10.0)) < 1e-3,
            "the corner (10, 0, 10), found {back}"
        );
    }

    #[test]
    fn only_contacts_within_the_radius_touch() {
        let kept = KeepClear {
            at_mm: vec![Vec3::ZERO],
            radius_mm: 2.0,
        };
        let contacts = [Vec3::new(1.5, 0.0, 0.0), Vec3::new(0.0, 2.5, 0.0)];
        assert_eq!(kept.touched(&contacts, Quat::IDENTITY, Vec3::ZERO), 1);
        let moved = kept.touched(&contacts, Quat::IDENTITY, Vec3::new(0.0, 2.0, 0.0));
        assert_eq!(moved, 1, "the point moved 2 mm toward the second contact");
    }
}
