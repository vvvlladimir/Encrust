use core_geometry::{Scalar, Vec3};
use printer_profiles::SupportProfile;

use super::joint::{highest_under_branches, shift};
use crate::Landing;
use crate::column::{MAX_LEAN_PER_DROP, MIN_PILLAR_HEIGHT_MM};
use crate::space::Space;
use crate::tree::SupportTree;

/// Carries the column of `tree` by `step`: across the plate its foot, its trunk upright over it
/// on a knee what it carries leans to at 45 degrees, or on the model the whole column, its foot
/// sliding along; up and down its top, unless a head hangs there, no higher than its branches
/// allow and no lower than the shortest pillar.
pub(super) fn carry_column(
    tree: &mut SupportTree,
    space: Space,
    step: Vec3,
    profile: &SupportProfile,
) {
    let across = step.with_z(0.0);
    // A head hangs from a root that is its neck: slid whole, it would turn off its face.
    if across != Vec3::ZERO && tree.landing().on_model && !holds_a_head(tree) {
        slide_column(tree, space, across);
    } else if across != Vec3::ZERO {
        // A joint carried rigidly would lean its branches past 45 degrees (ADR 0432, 0437).
        let from = space.plate(tree.landing().base);
        move_foot(tree, space, across);
        knee_over_foot(tree, space, from, profile);
    }
    if step.z == 0.0 || holds_a_head(tree) {
        return;
    }
    let root = tree.root_index();
    let top = space.node(tree, root);
    let highest = highest_under_branches(tree, root, top.truncate(), space);
    let lowest = lowest_top_mm(&space.plate_landing(tree.landing()), profile);
    let z = (top.z + step.z).min(highest).max(lowest);
    shift(tree, root, space, Vec3::Z * (z - top.z));
}

/// Whether the root of `tree` is a tip, or the neck a tip's head hangs from.
fn holds_a_head(tree: &SupportTree) -> bool {
    let root = tree.root_index();
    tree.root().is_leaf()
        || tree
            .nodes()
            .iter()
            .any(|node| node.parent == Some(root) && node.is_leaf())
}

/// The lowest the top of a column standing on `landing` (plate) may stand: the shortest pillar
/// over where its body ends, plate mm.
pub(super) fn lowest_top_mm(landing: &Landing, profile: &SupportProfile) -> Scalar {
    landing.pillar_bottom_z(profile) + MIN_PILLAR_HEIGHT_MM
}

/// Carries the column of `tree` standing on the model `across` the plate whole: its trunk
/// stays upright and `carried` stands it again on whatever is under it there.
fn slide_column(tree: &mut SupportTree, space: Space, across: Vec3) {
    shift(tree, tree.root_index(), space, across);
    let base = space.plate(tree.landing().base) + across;
    space.put_foot(tree, base);
}

/// Carries a tree's foot by `step`, keeping it on the plate: a foot in the air holds
/// nothing up.
fn move_foot(tree: &mut SupportTree, space: Space, step: Vec3) {
    let standing = space.plate(tree.landing().base) + step;
    space.put_foot(tree, standing.with_z(0.0));
}

/// Stands the trunk of `tree` straight over its foot, moved from `from` (plate mm), on a knee
/// the rest leans down to at 45 degrees: the knee that stood over `from`, or a new one.
fn knee_over_foot(tree: &mut SupportTree, space: Space, from: Vec3, profile: &SupportProfile) {
    let base = space.plate(tree.landing().base);
    let root = tree.root_index();
    let nodes = tree.nodes();
    let mut children = (0..nodes.len()).filter(|node| nodes[*node].parent == Some(root));
    let only = children.next().filter(|_| children.next().is_none());
    let knee = only.filter(|child| {
        !nodes[*child].is_leaf()
            && (space.node(tree, root) - from).truncate().length() < KNEE_OVER_FOOT_MM
    });
    let top = space.node(tree, knee.unwrap_or(root));
    if knee.is_none() {
        let radius_mm = tree.root().radius_mm;
        tree.stand_on_knee(tree.root().position, radius_mm);
    }
    // A foot dragged further than a 45-degree lean reaches stops where it does.
    let across = (base - top).truncate();
    let lowest = lowest_top_mm(&space.plate_landing(tree.landing()), profile);
    let most_mm = (top.z - lowest).max(0.0) * MAX_LEAN_PER_DROP;
    let across = across.clamp_length_max(most_mm);
    let base = top.truncate() + across;
    space.put_foot(tree, base.extend(0.0));
    let z = (top.z - across.length() / MAX_LEAN_PER_DROP).min(top.z);
    space.put(tree, tree.root_index(), base.extend(z));
}

/// How near straight over its foot a root must stand to be the knee a drag moves with it, mm.
const KNEE_OVER_FOOT_MM: Scalar = 1e-3;
