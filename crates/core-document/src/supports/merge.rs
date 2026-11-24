//! Supports of an object joined into one, named by points near their contacts, as the window's
//! Merge into one joins those picked (ADR 0440).

use std::sync::Arc;

use core_edit::ObjectId;
use core_engine::MERGE_NAMING_MM;
use core_geometry::{Bvh, Vec3};
use core_supports::{Placed, Profiles, SupportTree, merged};

use super::groups::table_of;
use super::kept_supports;
use crate::apply::{Applied, Report};
use crate::document::Document;
use crate::error::ApplyError;

/// Joins the supports of object `id` that carry the contacts nearest each of `near_mm` into one
/// tree, the rest grown again beside it.
pub(crate) fn apply_merge(
    document: &mut Document,
    id: ObjectId,
    near_mm: &[Vec3],
) -> Result<Applied, ApplyError> {
    let (state, meshes) = document.object(id).ok_or(ApplyError::NoObject(id))?;
    let plate = document.support_state();
    let table = table_of(state, &plate.groups, plate.raft);
    let profiles = Profiles::new(&table).ok_or(ApplyError::NoGroup(id))?;
    let (source, surface) = (&meshes.source, meshes.printed());
    let bvh = Arc::new(Bvh::build(surface));
    let transform = state.transform;
    let mut supports = kept_supports(state, meshes, &bvh, &table);
    let at = near_mm.first().copied().unwrap_or_default();
    let picked = picked_trees(supports.trees(), near_mm, id)?;
    let frozen: Vec<usize> = supports
        .freeze_all(&picked, transform)
        .into_iter()
        .flatten()
        .collect();
    let keep_out = supports.keep_out(source, transform);
    let placed = Placed::new(surface, &bvh, transform).blocking(keep_out.as_ref());
    let tree = merged(supports.frozen(), &frozen, &placed, profiles).map_err(|why| {
        ApplyError::MergeRefused {
            object: id,
            at_mm: at,
            why,
        }
    })?;
    supports.replace_frozen(&frozen, tree);
    if let Some(growth) = supports.growth(transform, &table) {
        supports.take_grown(growth.grow(source, surface, &bvh));
    }
    let kept = (
        supports.points().to_vec(),
        supports.frozen().to_vec(),
        supports.grown(transform),
    );
    document.set_every_support(id, kept);
    Ok(Applied {
        removed: Vec::new(),
        added: Vec::new(),
        report: Report::Merged,
    })
}

/// The trees of `trees` (plate) the points `near_mm` name, each once; refused where a point
/// names none, or all name one.
fn picked_trees(
    trees: &[SupportTree],
    near_mm: &[Vec3],
    id: ObjectId,
) -> Result<Vec<usize>, ApplyError> {
    let mut picked = (near_mm.iter())
        .map(|near| {
            support_near(trees, *near).ok_or(ApplyError::NoSupportNear {
                object: id,
                at_mm: *near,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    picked.sort_unstable();
    picked.dedup();
    if picked.len() < 2 {
        return Err(ApplyError::OneSupportToMerge {
            object: id,
            at_mm: near_mm.first().copied().unwrap_or_default(),
        });
    }
    Ok(picked)
}

/// The tree of `trees` (plate) carrying the contact nearest `at`, within `MERGE_NAMING_MM`.
pub fn support_near(trees: &[SupportTree], at: Vec3) -> Option<usize> {
    (trees.iter().enumerate())
        .flat_map(|(index, tree)| {
            tree.contacts()
                .map(move |contact| (index, contact.distance(at)))
        })
        .filter(|(_, distance)| *distance <= MERGE_NAMING_MM)
        .min_by(|one, other| one.1.total_cmp(&other.1))
        .map(|(index, _)| index)
}

#[cfg(test)]
mod tests;
