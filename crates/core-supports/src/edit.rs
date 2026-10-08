use core_geometry::{Mat4, Scalar, Vec3};
use printer_profiles::SupportProfile;

use crate::clear::{Beam, clear};
use crate::column::foot_clear;
use crate::pick::Part;
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

/// `tree` carried by `step`, or `None` when that is a step the model refuses.
///
/// Every part named in `parts` takes the same step, in plate millimetres. The tree is
/// then held to the rule the automatic run places by (ADR 0095): each tip is pulled back
/// onto the nearest surface it holds, and the whole of it is swept for clearance. `tree`
/// is in the model's own space, which is the space the answer comes back in.
pub fn carried(
    tree: &SupportTree,
    parts: &[Part],
    step: Vec3,
    placed: &Placed,
    profile: &SupportProfile,
) -> Option<SupportTree> {
    let matrix = placed.transform.to_matrix();
    if matrix.determinant().abs() < SINGULAR {
        return None;
    }
    let inverse = matrix.inverse();

    let mut carried = tree.clone();
    for part in parts {
        carry_part(&mut carried, *part, matrix, inverse, step);
    }
    snap_tips(&mut carried, placed, matrix, inverse);
    fits(&carried.moved(matrix), placed, profile).then_some(carried)
}

/// Moves one part of `tree`, which is kept in the model's own space while the step is a
/// step across the plate.
fn carry_part(tree: &mut SupportTree, part: Part, matrix: Mat4, inverse: Mat4, step: Vec3) {
    let carried = |tree: &mut SupportTree, node: usize| {
        let Some(from) = tree.nodes().get(node).map(|node| node.position) else {
            return;
        };
        tree.move_node(
            node,
            inverse.transform_point3(matrix.transform_point3(from) + step),
        );
    };

    match part {
        Part::Node(node) => carried(tree, node),
        Part::Strut(child) => {
            if let Some(parent) = tree.nodes().get(child).and_then(|node| node.parent) {
                carried(tree, parent);
            }
            carried(tree, child);
        }
        Part::Trunk => {
            carried(tree, tree.root_index());
            move_foot(tree, matrix, inverse, step);
        }
        Part::Foot => move_foot(tree, matrix, inverse, step),
    }
}

/// Carries a tree's foot by `step`, keeping it on the plate: a foot in the air holds
/// nothing up.
fn move_foot(tree: &mut SupportTree, matrix: Mat4, inverse: Mat4, step: Vec3) {
    let standing = matrix.transform_point3(tree.landing().base) + step;
    tree.move_landing(inverse.transform_point3(Vec3::new(standing.x, standing.y, 0.0)));
}

/// Pulls every tip back onto the surface it holds up: a support that has stopped touching
/// the model holds nothing, and one carried into it is not printable.
fn snap_tips(tree: &mut SupportTree, placed: &Placed, matrix: Mat4, inverse: Mat4) {
    let tips: Vec<(usize, Vec3)> = tree
        .nodes()
        .iter()
        .enumerate()
        .filter(|(_, node)| node.is_leaf())
        .map(|(index, node)| (index, node.position))
        .collect();
    for (index, position) in tips {
        if let Some(on) = on_model(placed, matrix.transform_point3(position)) {
            tree.move_node(index, inverse.transform_point3(on));
        }
    }
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

    /// A slab floating 10 mm over the plate, held up from below.
    fn shelf() -> Mesh {
        box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(10.0, 10.0, 20.0))
    }

    /// The one support under the middle of the shelf, touching its underside.
    fn under_the_shelf(model: &Mesh) -> crate::SupportTree {
        trees_on(
            &[SupportPoint::new(Vec3::new(5.0, 5.0, 10.0))],
            model,
            Transform::default(),
            Profiles::single(&profile()),
        )
        .pop()
        .expect("a column under the shelf stands")
    }

    /// Where the one tip of a single column sits in the tree.
    fn tip_of(tree: &crate::SupportTree) -> usize {
        tree.nodes()
            .iter()
            .position(crate::TreeNode::is_leaf)
            .expect("a column has a tip")
    }

    #[test]
    fn a_foot_carried_across_the_plate_stays_on_it() {
        let model = shelf();
        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());
        let tree = under_the_shelf(&model);

        let moved = carried(
            &tree,
            &[Part::Foot],
            Vec3::new(3.0, 0.0, 4.0),
            &placed,
            &profile(),
        )
        .expect("a foot slid along the plate under the shelf stands clear");

        let base = moved.landing().base;
        assert!(
            base.z.abs() < 1e-4,
            "a foot carried up still stands on the plate at z = 0, got {base}"
        );
        assert!(
            (base.x - (tree.landing().base.x + 3.0)).abs() < 1e-4,
            "the foot took the step across, got {base}"
        );
    }

    #[test]
    fn a_tip_carried_off_the_model_is_pulled_back_onto_it() {
        let model = shelf();
        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());
        let tree = under_the_shelf(&model);
        let tip = tip_of(&tree);

        let moved = carried(
            &tree,
            &[Part::Node(tip)],
            Vec3::new(0.0, 0.0, -2.0),
            &placed,
            &profile(),
        )
        .expect("a tip pulled back onto the shelf holds it up again");

        let at = moved.nodes()[tip].position;
        assert!(
            (at.z - 10.0).abs() < 1e-4,
            "the nearest surface is the shelf's underside at z = 10, got {at}"
        );
    }

    #[test]
    fn a_part_carried_into_the_model_is_refused() {
        let model = cube();
        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());
        let tree = beside(&model, 20.0);

        // The foot dragged 15 mm back lands under the cube, so the trunk would run
        // through the solid.
        assert!(
            carried(
                &tree,
                &[Part::Foot],
                Vec3::new(-15.0, 0.0, 0.0),
                &placed,
                &profile()
            )
            .is_none()
        );
    }

    #[test]
    fn a_flattened_model_carries_nothing() {
        let model = shelf();
        let bvh = Bvh::build(&model);
        let flat = Transform {
            scale: Vec3::new(1.0, 1.0, 0.0),
            ..Transform::default()
        };
        let placed = Placed::new(&model, &bvh, flat);
        let tree = under_the_shelf(&model);

        assert!(carried(&tree, &[Part::Foot], Vec3::X, &placed, &profile()).is_none());
    }
}
