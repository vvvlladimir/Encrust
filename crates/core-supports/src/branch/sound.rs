use super::carriers::{Carriers, THICKER_BY_MM, meeting_on};
use super::grow_one;
use super::small::small_pillar;
use crate::Landing;
use crate::clear::{Beam, clear};
use crate::column::Column;
use crate::group::Profiles;
use crate::mesher::{Fault, faults, is_small, sound};
use crate::placed::Placed;
use crate::tree::SupportTree;
use core_geometry::{FastMap, Vec2};
use rayon::prelude::*;

/// Runs every small-pillar tree on the model into a wider support in reach, no further off than
/// the part under it, and builds it thin all the way up; asked last of all groups (ADR 0251).
pub(super) fn stand_small_pillars_on_supports(
    forest: &mut [SupportTree],
    carriers: &Carriers,
    placed: &Placed,
    profiles: Profiles,
) {
    forest.par_iter_mut().for_each(|tree| {
        let profile = profiles.of(tree.group());
        let root = *tree.root();
        if !profile.branching.enabled || root.is_leaf() || !is_small(tree, profile) {
            return;
        }
        let radius_mm = profile.small_pillar.radius_mm().min(root.radius_mm);
        // A thin strut longer than the drop to the part sways and marks no less.
        let own_mm = root.position.distance(tree.landing().base);
        let met = carriers
            .near(root.position)
            .into_iter()
            .map(|index| &carriers.all[index])
            .filter(|carrier| carrier.radius_mm >= radius_mm + THICKER_BY_MM)
            .filter_map(|carrier| meeting_on(root.position, carrier, profile))
            .filter(|meet| root.position.distance(*meet) <= own_mm)
            .filter(|meet| {
                let strut = Beam {
                    from: root.position,
                    to: *meet,
                    from_radius_mm: radius_mm,
                    to_radius_mm: radius_mm,
                }
                .with_clearance(profile.clearance_mm)
                .trimmed(radius_mm + profile.clearance_mm);
                clear(placed, &strut)
            })
            .max_by(|a, b| a.z.total_cmp(&b.z));
        if let Some(meet) = met {
            let mut moved = tree.clone();
            moved.set_radius_mm(radius_mm);
            moved.stand_on(Landing {
                base: meet,
                on_model: true,
                pad_offset: Vec2::ZERO,
                pad_cut: None,
                on_support: true,
            });
            if sound(&moved, profiles, placed) {
                *tree = moved;
            }
        }
    });
}

/// `trees` with every tree whose bodies meet the model past their tips mended: faulty heads stand
/// alone, a trunk splits at its lowest joint, after [`MAX_REGROWS`] tip by tip (ADR 0337).
pub(super) fn only_sound(
    trees: Vec<SupportTree>,
    columns: &[Column],
    placed: &Placed,
    profiles: Profiles,
    carriers: &Carriers,
) -> Vec<SupportTree> {
    let mender = Mender {
        columns: columns
            .iter()
            .map(|column| (column.point, *column))
            .collect(),
        placed,
        profiles,
        carriers,
    };
    // Rounds in the order a queue would take them: each tree of a round is mended on its own,
    // so a round runs at once and what it keeps and regrows is joined in order.
    let mut kept = Vec::with_capacity(trees.len());
    let mut round: Vec<SupportTree> = trees;
    for regrown in 0..=MAX_REGROWS {
        let mended: Vec<(Vec<SupportTree>, Vec<SupportTree>)> = round
            .into_par_iter()
            .map(|tree| mender.mend(tree, regrown))
            .collect();
        round = Vec::new();
        for (sound, again) in mended {
            kept.extend(sound);
            round.extend(again);
        }
    }
    kept
}

/// What a tree that failed is grown again from.
struct Mender<'a> {
    columns: FastMap<usize, Column>,
    placed: &'a Placed<'a>,
    profiles: Profiles<'a>,
    carriers: &'a Carriers,
}

impl Mender<'_> {
    /// `tree` as it stands if sound, else what stands in its place and what is to be checked
    /// again, `regrown` times grown already.
    fn mend(&self, tree: SupportTree, regrown: u32) -> (Vec<SupportTree>, Vec<SupportTree>) {
        let Some(fault) = faults(&tree, self.profiles, self.placed) else {
            return (vec![tree], Vec::new());
        };
        let points: Vec<usize> = tree.points().collect();
        if points.len() < 2 {
            return (Vec::new(), Vec::new());
        }
        if regrown >= MAX_REGROWS {
            let alone = points.into_iter().flat_map(|point| self.alone(point));
            return (alone.collect(), Vec::new());
        }
        let (sound, regrow) = match fault {
            Fault::Trunk => (Vec::new(), halves(&tree)),
            Fault::Heads(heads) => {
                let alone = heads.iter().flat_map(|point| self.alone(*point)).collect();
                let rest = points
                    .into_iter()
                    .filter(|point| !heads.contains(point))
                    .collect();
                (alone, vec![rest])
            }
        };
        let again = regrow.iter().flat_map(|group| self.grow(group)).collect();
        (sound, again)
    }

    /// The trees the columns of `points` grow into together.
    fn grow(&self, points: &[usize]) -> Vec<SupportTree> {
        let columns: Vec<Column> = points
            .iter()
            .filter_map(|point| self.columns.get(point).copied())
            .collect();
        if columns.is_empty() {
            return Vec::new();
        }
        grow_one(&columns, self.placed, self.profiles, self.carriers)
    }

    /// The column of `point` grown on its own if that is sound, else a small pillar holding it.
    fn alone(&self, point: usize) -> Vec<SupportTree> {
        let mut grown = self.grow(&[point]);
        grown.retain(|tree| sound(tree, self.profiles, self.placed));
        if grown.is_empty()
            && let Some(column) = self.columns.get(&point)
        {
            grown.extend(small_pillar(
                column.contact,
                point,
                column.group,
                self.placed,
                self.profiles,
            ));
        }
        grown
    }
}

/// The points under each branch of the lowest joint of `tree`, where it splits in two.
pub(super) fn halves(tree: &SupportTree) -> Vec<Vec<usize>> {
    let nodes = tree.nodes();
    let children = |of: usize| -> Vec<usize> {
        (0..nodes.len())
            .filter(|&node| nodes[node].parent == Some(of))
            .collect()
    };
    let mut joint = tree.root_index();
    let mut below = children(joint);
    while below.len() == 1 {
        joint = below[0];
        below = children(joint);
    }
    below
        .into_iter()
        .map(|branch| {
            let mut under = vec![branch];
            let mut index = 0;
            while index < under.len() {
                under.extend(children(under[index]));
                index += 1;
            }
            under
                .into_iter()
                .filter_map(|node| nodes[node].point)
                .collect()
        })
        .collect()
}

/// How many times a tree is grown again without the tips that made it unsound before it
/// is stood tip by tip instead.
pub(super) const MAX_REGROWS: u32 = 3;
