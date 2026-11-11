use core_geometry::{Mat4, Scalar, Vec3};
use printer_profiles::SupportProfile;

use super::joint::{highest_under_branches, shift};
use crate::column::{MAX_LEAN_PER_DROP, MIN_PILLAR_HEIGHT_MM, PLATE_Z_MM};
use crate::tree::SupportTree;

/// Carries the column of `tree` by `step`: across the plate its foot, its trunk upright over it
/// on a knee what it carries leans to at 45 degrees; up and down its top, unless a head hangs
/// there, no higher than its branches allow and no lower than the shortest pillar.
pub(super) fn carry_column(
    tree: &mut SupportTree,
    space: (Mat4, Mat4),
    step: Vec3,
    profile: &SupportProfile,
) {
    let (matrix, inverse) = space;
    let across = step.with_z(0.0);
    if across != Vec3::ZERO {
        // A joint carried rigidly would lean its branches past 45 degrees (ADR 0432, 0437).
        let from = matrix.transform_point3(tree.landing().base);
        move_foot(tree, matrix, inverse, across);
        knee_over_foot(tree, space, from, profile);
    }
    let root = tree.root_index();
    let holds_a_head = tree
        .nodes()
        .iter()
        .any(|node| node.parent == Some(root) && node.is_leaf());
    if step.z == 0.0 || holds_a_head || tree.root().is_leaf() {
        return;
    }
    let top = matrix.transform_point3(tree.root().position);
    let highest = highest_under_branches(tree, root, top.truncate(), matrix);
    let z = (top.z + step.z).min(highest).max(lowest_top_mm(profile));
    shift(tree, root, space, Vec3::Z * (z - top.z));
}

/// The lowest the top of a column on the plate may stand: over its foot and the shortest
/// pillar, plate mm.
fn lowest_top_mm(profile: &SupportProfile) -> Scalar {
    PLATE_Z_MM + profile.footing_height_mm() + MIN_PILLAR_HEIGHT_MM
}

/// Carries a tree's foot by `step`, keeping it on the plate: a foot in the air holds
/// nothing up.
fn move_foot(tree: &mut SupportTree, matrix: Mat4, inverse: Mat4, step: Vec3) {
    let standing = matrix.transform_point3(tree.landing().base) + step;
    tree.move_landing(inverse.transform_point3(Vec3::new(standing.x, standing.y, 0.0)));
}

/// Stands the trunk of `tree` straight over its foot, moved from `from` (plate mm), on a knee
/// the rest leans down to at 45 degrees: the knee that stood over `from`, or a new one.
fn knee_over_foot(
    tree: &mut SupportTree,
    (matrix, inverse): (Mat4, Mat4),
    from: Vec3,
    profile: &SupportProfile,
) {
    let base = matrix.transform_point3(tree.landing().base);
    let root = tree.root_index();
    let nodes = tree.nodes();
    let plate = |node: usize| matrix.transform_point3(nodes[node].position);
    let mut children = (0..nodes.len()).filter(|node| nodes[*node].parent == Some(root));
    let only = children.next().filter(|_| children.next().is_none());
    let knee = only.filter(|child| {
        !nodes[*child].is_leaf() && (plate(root) - from).truncate().length() < KNEE_OVER_FOOT_MM
    });
    let top = knee.map_or_else(|| plate(root), plate);
    if knee.is_none() {
        let radius_mm = tree.root().radius_mm;
        tree.stand_on_knee(tree.root().position, radius_mm);
    }
    // A foot dragged further than a 45-degree lean reaches stops where it does.
    let across = (base - top).truncate();
    let most_mm = (top.z - lowest_top_mm(profile)).max(0.0) * MAX_LEAN_PER_DROP;
    let across = across.clamp_length_max(most_mm);
    let base = top.truncate() + across;
    tree.move_landing(inverse.transform_point3(base.extend(0.0)));
    let z = (top.z - across.length() / MAX_LEAN_PER_DROP).min(top.z);
    tree.move_node(tree.root_index(), inverse.transform_point3(base.extend(z)));
}

/// How near straight over its foot a root must stand to be the knee a drag moves with it, mm.
const KNEE_OVER_FOOT_MM: Scalar = 1e-3;
