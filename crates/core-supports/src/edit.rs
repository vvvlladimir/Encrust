use core_geometry::{Scalar, Vec3};
use printer_profiles::SupportProfile;

use crate::clear::{Beam, clear};
use crate::column::foot_clear;
use crate::placed::Placed;
use crate::tree::SupportTree;

/// A transform this close to singular has flattened its model, and there is no surface
/// left to put a tip on.
const SINGULAR: Scalar = 1e-12;

/// Whether every body of `tree` keeps out of the model, tip apart.
///
/// The same rule the automatic run places by (ADR 0077): each strut, the trunk and the
/// foot are swept as beams of their own radius with the profile's clearance around them,
/// and only the head of a tip is let through, because a tip is meant to touch. `tree` is
/// in plate coordinates, the space `placed` puts the model in.
pub fn fits(tree: &SupportTree, placed: &Placed, profile: &SupportProfile) -> bool {
    let head_mm = profile.top_length_mm() + profile.contact_depth_mm();
    let struts = tree.struts().all(|(child, parent)| {
        let nodes = tree.nodes();
        let beam = Beam {
            from: nodes[child].position,
            to: nodes[parent].position,
            from_radius_mm: nodes[child].radius_mm,
            to_radius_mm: nodes[parent].radius_mm,
        }
        .with_clearance(profile.clearance_mm)
        .trimmed(if nodes[child].is_leaf() { head_mm } else { 0.0 });
        clear(placed, &beam)
    });

    let root = tree.root();
    let landing = tree.landing();
    let trunk = Beam {
        from: root.position,
        to: Vec3::new(
            landing.base.x,
            landing.base.y,
            landing.pillar_bottom_z(profile),
        ),
        from_radius_mm: root.radius_mm,
        to_radius_mm: root.radius_mm,
    }
    .with_clearance(profile.clearance_mm)
    .trimmed(if root.is_leaf() { head_mm } else { 0.0 });

    struts
        && clear(placed, &trunk)
        && (landing.on_model || foot_clear(placed, landing.base, profile))
}

/// The nearest point of the model to `at`, in plate coordinates, for a tip that has to
/// keep touching what it holds up.
///
/// `None` when the model is nowhere near, or when the surface it came down on is painted
/// out of bounds: a tip may not be dropped onto a blocked face any more by hand than by
/// the automatic run (ADR 0093).
pub fn on_model(placed: &Placed, at: Vec3) -> Option<Vec3> {
    let matrix = placed.transform.to_matrix();
    if matrix.determinant().abs() < SINGULAR {
        return None;
    }
    let inverse = matrix.inverse();
    let closest = placed
        .bvh
        .closest(placed.model, inverse.transform_point3(at))?;
    (!placed.blocks_face(closest.face)).then(|| matrix.transform_point3(closest.point))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{box_mesh, profile, trees_on};
    use crate::{Profiles, SupportPoint};
    use core_geometry::{Bvh, Mesh, Transform};

    /// A cube spanning 0..10 on every axis, standing on the plate.
    fn cube() -> Mesh {
        box_mesh(Vec3::ZERO, Vec3::splat(10.0))
    }

    /// One support standing in clear air beside the cube, 20 mm up.
    fn beside(model: &Mesh, x: Scalar) -> crate::SupportTree {
        trees_on(
            &[SupportPoint::new(Vec3::new(x, 5.0, 20.0))],
            model,
            Transform::default(),
            Profiles::single(&profile()),
        )
        .pop()
        .expect("a column in clear air stands")
    }

    #[test]
    fn a_support_standing_clear_of_the_model_fits() {
        let model = cube();
        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());
        assert!(fits(&beside(&model, 20.0), &placed, &profile()));
    }

    #[test]
    fn a_support_carried_into_the_model_does_not_fit() {
        let model = cube();
        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());

        // Standing clear, then its foot dragged under the cube: the trunk now runs
        // through the solid.
        let mut tree = beside(&model, 20.0);
        tree.move_landing(Vec3::new(5.0, 5.0, 0.0));

        assert!(
            !fits(&tree, &placed, &profile()),
            "a trunk through the model is what the automatic run refuses too"
        );
    }

    #[test]
    fn a_tip_is_pulled_onto_the_surface_it_holds() {
        let model = cube();
        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());

        let on = on_model(&placed, Vec3::new(5.0, 5.0, 14.0)).expect("the cube is under it");
        assert!(
            (on.z - 10.0).abs() < 1e-4 && (on.x - 5.0).abs() < 1e-4,
            "the nearest surface is the lid at z = 10, got {on}"
        );
    }

    #[test]
    fn a_tip_is_not_pulled_onto_a_blocked_face() {
        let model = cube();
        let bvh = Bvh::build(&model);
        let mut region = crate::Region::default();
        for face in 0..model.faces.len() {
            region.set(face, true);
        }
        let blocked = crate::Blocked::new(&model, &region, Transform::default())
            .expect("the cube was painted");
        let placed = Placed::new(&model, &bvh, Transform::default()).blocking(Some(&blocked));

        assert!(on_model(&placed, Vec3::new(5.0, 5.0, 14.0)).is_none());
    }
}
