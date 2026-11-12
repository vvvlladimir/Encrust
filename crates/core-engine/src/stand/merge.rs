use std::collections::HashSet;

use core_geometry::{Scalar, Vec3};
use core_supports::{Column, Placed, Profiles, Refused, SupportTree, grow_beside, merged};

use crate::error::EngineError;

/// How far from a support's contact a point may be and still name it, mm: a contact copied from
/// `--json` at its printed precision, well inside the closest spacing of two contacts.
const NAMING_MM: Scalar = 0.5;

/// `trees` (plate) with the supports each of `merges` names by their points' `contacts` joined
/// into one, as the window's Merge into one joins them; the rest grow again beside them (ADR 0440).
pub(super) fn merged_near(
    trees: Vec<SupportTree>,
    (merges, contacts): (&[Vec<Vec3>], &[Vec3]),
    columns: &[Column],
    placed: &Placed,
    profiles: Profiles,
) -> Result<Vec<SupportTree>, EngineError> {
    if merges.is_empty() {
        return Ok(trees);
    }
    let (mut grown, mut joined) = (trees, Vec::new());
    for near in merges {
        let at = near.first().copied().unwrap_or_default();
        let standing: Vec<SupportTree> = grown.iter().chain(&joined).cloned().collect();
        let mut picked = near
            .iter()
            .map(|at| named(&standing, contacts, *at).ok_or(EngineError::NoSupportNear { at: *at }))
            .collect::<Result<Vec<_>, _>>()?;
        picked.sort_unstable();
        picked.dedup();
        if picked.len() < 2 {
            return Err(EngineError::OneSupportToMerge { at });
        }
        let tree = merged(&standing, &picked, placed, profiles)
            .map_err(|why| EngineError::MergeRefused { at, why })?;
        for index in picked.iter().rev() {
            match index.checked_sub(grown.len()) {
                None => grown.remove(*index),
                Some(earlier) => joined.remove(earlier),
            };
        }
        joined.push(tree);
    }
    let held: HashSet<usize> = joined.iter().flat_map(SupportTree::points).collect();
    let rest: Vec<Column> = (columns.iter())
        .filter(|column| !held.contains(&column.point))
        .copied()
        .collect();
    let mut trees = grow_beside(&rest, &joined, placed, profiles);
    trees.extend(joined);
    Ok(trees)
}

/// The support of `trees` carrying the point of `contacts` nearest `at`, within [`NAMING_MM`].
fn named(trees: &[SupportTree], contacts: &[Vec3], at: Vec3) -> Option<usize> {
    let (point, _) = (contacts.iter().enumerate())
        .map(|(point, contact)| (point, contact.distance(at)))
        .filter(|(_, distance)| *distance <= NAMING_MM)
        .min_by(|a, b| a.1.total_cmp(&b.1))?;
    trees
        .iter()
        .position(|tree| tree.points().any(|held| held == point))
}

/// What `why` says in an error a user reads.
pub(crate) fn merge_refusal(why: Refused) -> &'static str {
    match why {
        Refused::Collides => "the model is in the way",
        Refused::Leans => "a strut would lean past 45 degrees",
        Refused::NoFooting => "the trunk has nowhere to stand",
        Refused::NoHold => "a tip has no surface to hold",
    }
}

#[cfg(test)]
mod tests;
