use core_geometry::{Scalar, Vec2, Vec3};

use crate::column::MAX_LEAN_PER_DROP;
use crate::space::Space;
use crate::tree::SupportTree;

/// Moves node `node` of `tree` by `step`, plate mm, which bends the struts meeting there, then
/// settles it as low as its branches need (ADR 0438).
pub(super) fn carry_node(tree: &mut SupportTree, node: usize, space: Space, step: Vec3) {
    shift(tree, node, space, step);
    settle(tree, node, space);
}

/// Moves node `node` of `tree` by `step`, plate mm, and nothing else.
pub(super) fn shift(tree: &mut SupportTree, node: usize, space: Space, step: Vec3) {
    if node < tree.nodes().len() {
        let to = space.node(tree, node) + step;
        space.put(tree, node, to);
    }
}

/// Lowers joint `node` of `tree` until no branch above it leans past 45 degrees: a joint
/// carried aside slides down the cone its branches allow rather than being refused.
pub(super) fn settle(tree: &mut SupportTree, node: usize, space: Space) {
    if node >= tree.nodes().len() {
        return;
    }
    let at = space.node(tree, node);
    let highest = highest_under_branches(tree, node, at.truncate(), space);
    if at.z > highest {
        space.put(tree, node, at.with_z(highest));
    }
}

/// The highest a joint of `tree` standing over `across` (plate) may be for every strut from a
/// joint above down to it to lean no further than 45 degrees, mm; unbounded with none.
pub(super) fn highest_under_branches(
    tree: &SupportTree,
    node: usize,
    across: Vec2,
    space: Space,
) -> Scalar {
    tree.nodes()
        .iter()
        .filter(|above| above.parent == Some(node) && !above.is_leaf())
        .map(|above| {
            let above = space.plate(above.position);
            above.z - (above.truncate() - across).length() / MAX_LEAN_PER_DROP
        })
        .fold(Scalar::INFINITY, Scalar::min)
}
