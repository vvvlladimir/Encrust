use core_geometry::{Mat4, Scalar, Vec2, Vec3};
use printer_profiles::SupportProfile;

use super::column::lowest_top_mm;
use super::{Refused, allowed, strut_fit, trunk_fit};
use crate::column::MAX_LEAN_PER_DROP;
use crate::placed::Placed;
use crate::tree::SupportTree;

/// `after`, carried from `before` (both model space), with the joints beside what the carry
/// moved shifted a little where that lets it stand; where no such shift does, why `after` may
/// not (ADR 0440, 0441).
pub fn eased(
    before: &SupportTree,
    after: &SupportTree,
    placed: &Placed,
    profile: &SupportProfile,
) -> Result<SupportTree, Refused> {
    let Err(refused) = allowed(before, after, placed, profile) else {
        return Ok(after.clone());
    };
    let matrix = placed.transform.to_matrix();
    let inverse = placed.transform.inverse_matrix().ok_or(refused)?;
    let mut eased = after.clone();
    for joint in giving(before, after, placed, profile, matrix).ok_or(refused)? {
        give_way(&mut eased, joint, placed, profile, (matrix, inverse)).ok_or(refused)?;
    }
    allowed(before, &eased, placed, profile).map(|()| eased)
}

/// The joints the carry left in place at the far end of a strut it bent out of line, each
/// once; `None` when such a strut has no joint there free to move, or none needs one.
fn giving(
    before: &SupportTree,
    after: &SupportTree,
    placed: &Placed,
    profile: &SupportProfile,
    matrix: Mat4,
) -> Option<Vec<usize>> {
    let (was, now) = (before.moved(matrix), after.moved(matrix));
    let at = |tree: &SupportTree, node: usize| tree.nodes().get(node).map(|node| node.position);
    let moved = |node: usize| at(&was, node) != at(&now, node);
    let mut joints = Vec::new();
    for (child, parent) in now
        .struts()
        .filter(|(child, parent)| moved(*child) != moved(*parent))
    {
        if strut_fit(&now, child, parent, placed, profile).is_ok() {
            continue;
        }
        let free = if moved(child) { parent } else { child };
        if !may_give(&now, free) {
            return None;
        }
        if !joints.contains(&free) {
            joints.push(free);
        }
    }
    (!joints.is_empty() && joints.len() <= MOST_JOINTS).then_some(joints)
}

/// Whether `node` of `tree` may be shifted to make room: a joint, the root too unless a head
/// sunk in the plate ends there; one on the model is only lowered, see [`give_way`].
fn may_give(tree: &SupportTree, node: usize) -> bool {
    let is_joint = tree.nodes().get(node).is_some_and(|node| !node.is_leaf());
    is_joint && (node != tree.root_index() || !tree.landing().in_plate)
}

/// Moves joint `joint` of `tree` to the nearest place, aside or lower and within
/// [`MOST_GIVE_MM`], where every strut meeting it fits; `None` when none does. A root on the
/// model is only lowered: its trunk shortens over the foot it has (ADR 0441).
fn give_way(
    tree: &mut SupportTree,
    joint: usize,
    placed: &Placed,
    profile: &SupportProfile,
    (matrix, inverse): (Mat4, Mat4),
) -> Option<()> {
    let plate = tree.moved(matrix);
    let from = plate.nodes()[joint].position;
    let is_root = joint == plate.root_index();
    let level = if is_root && plate.landing().on_model {
        Vec::new()
    } else {
        level_shifts(&plate, joint)
    };
    // A joint lowered leans every branch above it less.
    let drops = RINGS_MM.map(|drop_mm| Vec3::Z * -drop_mm);
    let lowered = level.iter().map(|shift| *shift - Vec3::Z * shift.length());
    let mut shifts: Vec<Vec3> = level.iter().copied().chain(drops).chain(lowered).collect();
    shifts.retain(|shift| shift.length() <= MOST_GIVE_MM);
    shifts.sort_by(|a, b| a.length().total_cmp(&b.length()));
    let to = shifts.into_iter().map(|shift| from + shift).find(|to| {
        let mut trial = plate.clone();
        shift_joint(&mut trial, joint, *to);
        holds(&trial, joint, placed, profile)
    })?;
    tree.move_node(joint, inverse.transform_point3(to));
    // A foot on the model stays on it: the root over it was only lowered.
    if is_root && !tree.landing().on_model {
        let base = matrix.transform_point3(tree.landing().base);
        tree.move_landing(inverse.transform_point3(to.truncate().extend(base.z)));
    }
    Some(())
}

/// The level shifts tried for joint `joint` of `tree` (plate): those that bring each strut
/// meeting it back to 45 degrees, then rings of [`RING_DIRECTIONS`] at each of [`RINGS_MM`].
fn level_shifts(tree: &SupportTree, joint: usize) -> Vec<Vec3> {
    let nodes = tree.nodes();
    let at = nodes[joint].position;
    let leaning = nodes
        .iter()
        .enumerate()
        .filter(|(index, node)| node.parent == Some(joint) || nodes[joint].parent == Some(*index))
        .filter(|(_, node)| !node.is_leaf())
        .map(|(_, node)| {
            let reach = (at.z - node.position.z).abs() * MAX_LEAN_PER_DROP;
            let across = (at - node.position).truncate().clamp_length_max(reach);
            (node.position.truncate() + across - at.truncate()).extend(0.0)
        });
    let rings = RINGS_MM.iter().flat_map(|radius_mm| {
        (0..RING_DIRECTIONS).map(move |step| {
            let turn = std::f32::consts::TAU * step as Scalar / RING_DIRECTIONS as Scalar;
            (Vec2::from_angle(turn) * *radius_mm).extend(0.0)
        })
    });
    leaning
        .chain(rings)
        .filter(|shift| *shift != Vec3::ZERO)
        .collect()
}

/// Puts joint `joint` of `tree` at `to`, the root's foot on the plate under it.
fn shift_joint(tree: &mut SupportTree, joint: usize, to: Vec3) {
    tree.move_node(joint, to);
    if joint == tree.root_index() && !tree.landing().on_model {
        let base = tree.landing().base;
        tree.move_landing(to.truncate().extend(base.z));
    }
}

/// Whether every strut meeting joint `joint` of `tree` (plate) fits and still runs down from
/// child to parent, and the trunk under the root fits and stays no shorter than a pillar.
fn holds(tree: &SupportTree, joint: usize, placed: &Placed, profile: &SupportProfile) -> bool {
    let nodes = tree.nodes();
    let meeting = tree
        .struts()
        .filter(|(child, parent)| *child == joint || *parent == joint)
        .all(|(child, parent)| {
            nodes[child].position.z > nodes[parent].position.z
                && strut_fit(tree, child, parent, placed, profile).is_ok()
        });
    let column = joint != tree.root_index()
        || (nodes[joint].position.z >= lowest_top_mm(&tree.landing(), profile)
            && trunk_fit(tree, placed, profile).is_ok());
    meeting && column
}

/// The most joints one carry may shift to make room.
const MOST_JOINTS: usize = 4;
/// The furthest a joint is shifted to make room, mm: further is a different support.
const MOST_GIVE_MM: Scalar = 5.0;
/// The ring radii tried around a joint, mm, nearest first.
const RINGS_MM: [Scalar; 6] = [0.5, 1.0, 2.0, 3.0, 4.0, 5.0];
/// How many directions each ring tries.
const RING_DIRECTIONS: usize = 8;

#[cfg(test)]
mod tests;
