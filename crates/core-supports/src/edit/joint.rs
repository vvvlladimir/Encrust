use core_geometry::{Mat4, Scalar, Vec3};

use crate::column::MAX_LEAN_PER_DROP;
use crate::tree::SupportTree;

/// Moves node `node` of `tree` by `step`, plate mm, which bends the struts meeting there, then
/// settles it as low as its branches need (ADR 0438).
pub(super) fn carry_node(tree: &mut SupportTree, node: usize, space: (Mat4, Mat4), step: Vec3) {
    shift(tree, node, space, step);
    settle(tree, node, space);
}

/// Moves node `node` of `tree` by `step`, plate mm, and nothing else.
pub(super) fn shift(
    tree: &mut SupportTree,
    node: usize,
    (matrix, inverse): (Mat4, Mat4),
    step: Vec3,
) {
    if let Some(from) = tree.nodes().get(node).map(|node| node.position) {
        tree.move_node(
            node,
            inverse.transform_point3(matrix.transform_point3(from) + step),
        );
    }
}

/// Lowers joint `node` of `tree` until no branch above it leans past 45 degrees: a joint
/// carried aside slides down the cone its branches allow rather than being refused.
pub(super) fn settle(tree: &mut SupportTree, node: usize, (matrix, inverse): (Mat4, Mat4)) {
    let Some(at) = tree
        .nodes()
        .get(node)
        .map(|node| matrix.transform_point3(node.position))
    else {
        return;
    };
    let highest = highest_under_branches(tree, node, at.truncate(), matrix);
    if at.z > highest {
        tree.move_node(node, inverse.transform_point3(at.with_z(highest)));
    }
}

/// The highest a joint of `tree` standing over `across` (plate) may be for every strut from a
/// joint above down to it to lean no further than 45 degrees, mm; unbounded with none.
pub(super) fn highest_under_branches(
    tree: &SupportTree,
    node: usize,
    across: core_geometry::Vec2,
    matrix: Mat4,
) -> Scalar {
    tree.nodes()
        .iter()
        .filter(|above| above.parent == Some(node) && !above.is_leaf())
        .map(|above| {
            let above = matrix.transform_point3(above.position);
            above.z - (above.truncate() - across).length() / MAX_LEAN_PER_DROP
        })
        .fold(Scalar::INFINITY, Scalar::min)
}
