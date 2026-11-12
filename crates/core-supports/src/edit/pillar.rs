use core_geometry::{Ray, Scalar, Vec3};

use super::{Refused, allowed, on_model};
use crate::branch::grown_alone;
use crate::column::{SURFACE_EPSILON_MM, face_normal};
use crate::group::Profiles;
use crate::mesher::is_small;
use crate::pick::Part;
use crate::placed::Placed;
use crate::tree::SupportTree;

/// Whether `tree` (model space) is a small pillar, which a hand moves by its contact alone and
/// growing builds again from there (ADR 0442).
pub(super) fn is_pillar(tree: &SupportTree, placed: &Placed, profiles: Profiles) -> bool {
    is_small(
        &tree.moved(placed.transform.to_matrix()),
        profiles.of(tree.group()),
    )
}

/// Small pillar `tree` (model space) grown again from its contact put down at `contact`
/// (plate): a small pillar where one stands there, else a whole support (ADR 0442).
pub(super) fn regrown(
    tree: &SupportTree,
    contact: Vec3,
    placed: &Placed,
    profiles: Profiles,
) -> Result<SupportTree, Refused> {
    let inverse = placed.transform.inverse_matrix().ok_or(Refused::NoHold)?;
    let tip = tree
        .nodes()
        .iter()
        .find(|node| node.is_leaf())
        .ok_or(Refused::NoHold)?;
    let point = tip.point.ok_or(Refused::NoHold)?;
    let group = tip.head.unwrap_or(tree.group());
    let grown = grown_alone(contact, point, group, placed, profiles)
        .ok_or(Refused::NoFooting)?
        .moved(inverse);
    allowed(tree, &grown, placed, profiles.of(grown.group()))?;
    Ok(grown)
}

/// Small pillar `tree` (model space) with whichever `parts` held carried by `step` (plate mm):
/// its contact carried, onto a face looking down, and the pillar grown again from there.
pub(super) fn pillar_carried(
    tree: &SupportTree,
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
    let matrix = placed.transform.to_matrix();
    let tip = tree
        .nodes()
        .iter()
        .find(|node| node.is_leaf())
        .ok_or(Refused::NoHold)?;
    let at = matrix.transform_point3(tip.position) + step;
    let contact = held_over(placed, at).ok_or(Refused::NoHold)?;
    regrown(tree, contact, placed, profiles)
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
