use core_geometry::{Ray, Scalar, Vec3};

use super::{Refused, allowed, on_model};
use crate::branch::grown_alone;
use crate::column::{SURFACE_EPSILON_MM, face_normal};
use crate::group::Profiles;
use crate::mesher::is_small;
use crate::pick::Part;
use crate::placed::Placed;
use crate::space::Space;
use crate::tree::SupportTree;

/// Whether `tree` (model space) is a small pillar, which a hand moves by its contact alone and
/// growing builds again from there (ADR 0442).
pub(super) fn is_pillar(tree: &SupportTree, placed: &Placed, profiles: Profiles) -> bool {
    is_small(
        &tree.moved(placed.transform.to_matrix()),
        profiles.of(tree.group()),
    )
}

/// Small pillar `index` of `frozen` (model space) grown again from `contact` (plate) beside the
/// other frozen trees: a small pillar where one stands, else a whole support (ADR 0442, 0444).
pub(super) fn regrown(
    (frozen, index): (&[SupportTree], usize),
    contact: Vec3,
    placed: &Placed,
    profiles: Profiles,
) -> Result<SupportTree, Refused> {
    let tree = frozen.get(index).ok_or(Refused::NoHold)?;
    let space = Space::of(placed).ok_or(Refused::NoHold)?;
    let beside: Vec<SupportTree> = (frozen.iter().enumerate())
        .filter(|(other, _)| *other != index)
        .map(|(_, other)| space.plate_tree(other))
        .collect();
    let tip = tree
        .nodes()
        .iter()
        .find(|node| node.is_leaf())
        .ok_or(Refused::NoHold)?;
    let point = tip.point.ok_or(Refused::NoHold)?;
    let group = tip.head.unwrap_or(tree.group());
    let grown =
        grown_alone(contact, point, group, &beside, placed, profiles).ok_or(Refused::NoFooting)?;
    let grown = space.model_tree(&grown);
    allowed(tree, &grown, placed, profiles.of(grown.group()))?;
    Ok(grown)
}

/// Small pillar `index` of `frozen` (model space) with whichever `parts` held carried by `step`
/// (plate mm): its contact carried, onto a face looking down, and the pillar grown again there.
pub(super) fn pillar_carried(
    (frozen, index): (&[SupportTree], usize),
    parts: &[Part],
    step: Vec3,
    placed: &Placed,
    profiles: Profiles,
) -> Result<SupportTree, Refused> {
    // A foot stays on what it stands on, so only the step across moves it.
    let step = if parts.iter().all(|part| *part == Part::Foot) {
        step.with_z(0.0)
    } else {
        step
    };
    let space = Space::of(placed).ok_or(Refused::NoHold)?;
    let tip = (frozen.get(index).ok_or(Refused::NoHold)?.nodes().iter())
        .find(|node| node.is_leaf())
        .ok_or(Refused::NoHold)?;
    let at = space.plate(tip.position) + step;
    let contact = held_over(placed, at).ok_or(Refused::NoHold)?;
    regrown((frozen, index), contact, placed, profiles)
}

/// The point of a face looking down that a contact carried to `at` (plate) holds: the nearest
/// surface within [`HOLD_REACH_MM`] where it looks down, else the first one straight over `at`.
fn held_over(placed: &Placed, at: Vec3) -> Option<Vec3> {
    let looks_down = |point: Vec3| face_normal(placed, point).is_some_and(|normal| normal.z < 0.0);
    let near =
        on_model(placed, at).filter(|near| near.distance(at) <= HOLD_REACH_MM && looks_down(*near));
    near.or_else(|| {
        let up = Ray::new(at - Vec3::Z * SURFACE_EPSILON_MM, Vec3::Z);
        placed
            .raycast(&up)
            .filter(|hit| hit.normal.z < 0.0 && !placed.blocks_at(hit.point))
            .map(|hit| hit.point)
    })
}

/// How far from where it was carried a contact is put down on the nearest surface, mm: as far
/// as a carried foot looks round for its footing; further is a different support.
const HOLD_REACH_MM: Scalar = 2.0;

#[cfg(test)]
mod tests;
