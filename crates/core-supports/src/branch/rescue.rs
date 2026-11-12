use super::carriers::Carriers;
use super::grow_one;
use super::sound::only_sound;
use crate::column::{Column, column_at, moved, refitted_necks};
use crate::group::Profiles;
use crate::placed::Placed;
use crate::tree::SupportTree;
use core_geometry::FastSet;
use rayon::prelude::*;

/// Grows again alone each of `columns` no tree of `forest` carries, from a turned or cut head,
/// then a moved contact; the first sound one with room on what it stands on is kept (0431, 0444).
pub(super) fn rescue(
    forest: &mut Vec<SupportTree>,
    columns: &[Column],
    placed: &Placed,
    profiles: Profiles,
    carriers: &mut Carriers,
) {
    let held: FastSet<usize> = forest.iter().flat_map(SupportTree::points).collect();
    let lost: Vec<&Column> = columns
        .iter()
        .filter(|column| !held.contains(&column.point))
        .collect();
    let tried: Vec<Option<Vec<SupportTree>>> = lost
        .par_iter()
        .map(|column| rescued(column, placed, profiles, carriers))
        .collect();
    // In order: a rescue running into a support the ones before it filled is grown once more.
    for (column, trees) in lost.into_iter().zip(tried) {
        let trees = match trees {
            Some(trees) if fits_on(&trees, carriers, profiles) => Some(trees),
            Some(_) => rescued(column, placed, profiles, carriers),
            None => None,
        };
        if let Some(trees) = trees {
            carriers.take(&trees, profiles);
            forest.extend(trees);
        }
    }
}

/// The trees `column` grows into on its own from the first head or contact that stands sound.
fn rescued(
    column: &Column,
    placed: &Placed,
    profiles: Profiles,
    carriers: &Carriers,
) -> Option<Vec<SupportTree>> {
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
}

/// Whether every tree of `trees` standing on a support of `carriers` has room on it.
fn fits_on(trees: &[SupportTree], carriers: &Carriers, profiles: Profiles) -> bool {
    trees.iter().all(|tree| {
        let most = profiles.of(tree.group()).branching.max_tips_per_trunk as usize;
        carriers
            .host_of(tree)
            .is_none_or(|host| carriers.has_room(host, tree.tip_count(), most))
    })
}

#[cfg(test)]
mod tests;
