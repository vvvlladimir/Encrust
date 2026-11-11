use super::carriers::Carriers;
use super::grow_one;
use super::sound::only_sound;
use crate::column::{Column, column_at, moved, refitted_necks};
use crate::group::Profiles;
use crate::placed::Placed;
use crate::tree::SupportTree;
use core_geometry::FastSet;
use rayon::prelude::*;

/// Grows again, each on its own, the `columns` no tree of `forest` carries, from a head turned or
/// cut to fit, then from a contact moved along its face; the first that stands sound is kept
/// (ADR 0431).
pub(super) fn rescue(
    forest: &mut Vec<SupportTree>,
    columns: &[Column],
    placed: &Placed,
    profiles: Profiles,
    carriers: &Carriers,
) {
    let held: FastSet<usize> = forest.iter().flat_map(SupportTree::points).collect();
    let rescued: Vec<Vec<SupportTree>> = columns
        .par_iter()
        .filter(|column| !held.contains(&column.point))
        .filter_map(|column| {
            let Column {
                point,
                group,
                contact,
                ..
            } = *column;
            let profile = profiles.of(group);
            let turned = refitted_necks(placed, contact, profile).map(|neck| (contact, neck));
            turned
                .chain(moved(placed, contact, profile))
                .find_map(|head| {
                    let refit = column_at(placed, point, group, head, profile);
                    let trees = grow_one(&[refit], placed, profiles, carriers);
                    let trees = only_sound(trees, &[refit], placed, profiles, carriers);
                    trees
                        .iter()
                        .any(|tree| tree.points().any(|held| held == point))
                        .then_some(trees)
                })
        })
        .collect();
    forest.extend(rescued.into_iter().flatten());
}

#[cfg(test)]
mod tests;
