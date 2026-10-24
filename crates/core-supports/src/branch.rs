use std::collections::HashMap;

mod grid;

use crate::placed::Placed;
use core_geometry::{Buckets, Scalar, Vec3};
use printer_profiles::SupportProfile;

use crate::Landing;
use crate::base::braceable;
use crate::clear::{Beam, clear};
use crate::column::{Column, MAX_LEAN_PER_DROP, has_head, landing};
use crate::group::Profiles;
use crate::tree::{SupportTree, TreeNode};

mod carriers;
mod ground;
mod harvest;
mod merge;
mod sound;
use carriers::{Carriers, THICKER_BY_MM};
use sound::{only_sound, stand_small_pillars_on_supports};

/// Two tips this close together stand in the same place, and the direction between them
/// carries no information. Well under one pixel of any panel.
const SAME_PLACE_MM: Scalar = 1e-4;

/// Merges the tips of `columns` (plate coordinates) into shared trunks, highest meeting point
/// first (ADR 0040); same-width groups grow together, thickest first (ADR 0325, 0248).
pub fn grow(columns: &[Column], placed: &Placed, profiles: Profiles) -> Vec<SupportTree> {
    let reach_mm = profiles
        .groups()
        .map(|(_, profile)| profile.branching.max_merge_distance_mm)
        .fold(SAME_PLACE_MM, Scalar::max);

    let mut carriers = Carriers::new(reach_mm);
    let mut forest = Vec::new();
    for family in families(profiles) {
        let mine: Vec<Column> = columns
            .iter()
            .filter(|column| family.contains(&column.group))
            .copied()
            .collect();
        let mut trees = grow_one(&mine, placed, profiles, &carriers);
        // Before anything thinner is leant onto them: a trunk moved afterwards would
        // leave what joined it ending in the air. A trunk on the grid stays on its node.
        if on_grid(&family, profiles).is_none() {
            gather(&mut trees, &forest, placed, profiles);
        }
        let trees = only_sound(trees, &mine, placed, profiles, &carriers);
        carriers.take(&trees, profiles);
        forest.extend(trees);
    }
    stand_small_pillars_on_supports(&mut forest, &carriers, placed, profiles);
    forest.sort_by_key(SupportTree::group);
    forest
}

/// The groups that grow together, thickest pillar first: every group that branches with
/// the others of its pillar's width, and one that does not on its own.
fn families(profiles: Profiles) -> Vec<Vec<u16>> {
    let mut order: Vec<u16> = profiles.groups().map(|(group, _)| group).collect();
    order.sort_by(|a, b| {
        let width = |group: &u16| profiles.of(*group).pillar_radius_mm();
        width(b)
            .total_cmp(&width(a))
            .then_with(|| profiles.heavier_first(*a, *b))
    });
    let mut families: Vec<Vec<u16>> = Vec::new();
    for group in order {
        let profile = profiles.of(group);
        let joins = families.last().is_some_and(|family| {
            let first = profiles.of(family[0]);
            first.branching.enabled
                && profile.branching.enabled
                && first.pillar_radius_mm() - profile.pillar_radius_mm() < THICKER_BY_MM
        });
        match families.last_mut() {
            Some(family) if joins => family.push(group),
            _ => families.push(vec![group]),
        }
    }
    families
}

/// The grid a family's trunks stand on: the first group's, where every group of it has
/// one switched on.
fn on_grid(groups: &[u16], profiles: Profiles) -> Option<printer_profiles::Grid> {
    let first = profiles.of(*groups.first()?).grid;
    groups
        .iter()
        .all(|group| profiles.of(*group).grid.enabled)
        .then_some(first)
}

/// Grows one family's columns, each built to its own group's profile.
fn grow_one(
    columns: &[Column],
    placed: &Placed,
    profiles: Profiles,
    carriers: &Carriers,
) -> Vec<SupportTree> {
    let mut forest = Forest::seeded(columns, profiles, carriers);
    let branching = columns
        .iter()
        .all(|column| profiles.of(column.group).branching.enabled);
    let groups: Vec<u16> = columns.iter().map(|column| column.group).collect();
    if let Some(grid) = on_grid(&groups, profiles) {
        forest.snap_to_grid(placed, grid);
    } else if branching {
        forest.merge(placed);
        // Tips the model stands in the way of are bent aside before the second pass, so
        // that what they lean onto can share a trunk rather than stand alone.
        if forest.bend_blocked(placed) {
            forest.merge(placed);
        }
    }
    forest.harvest(placed)
}

/// How near a trunk that stood alone is brought to its neighbour, as a share of the
/// distance a brace spans: inside it with room to spare, and not on top of the other.
const GATHERED: Scalar = 0.75;

/// Leans every tall trunk out of a brace's reach towards its nearest neighbour, a knee within
/// 45 degrees, until one reaches; `settled` trunks count as neighbours (ADR 0240, 0250, 0252).
fn gather(trees: &mut [SupportTree], settled: &[SupportTree], placed: &Placed, profiles: Profiles) {
    let at = |tree: &SupportTree| tree.root().position.truncate();
    for index in 0..trees.len() {
        let tree = &trees[index];
        let profile = profiles.of(tree.group());
        let leans = profile.branching.enabled && profile.bracing.enabled;
        if !leans || tree.landing().on_model || !braceable(tree, profile) {
            continue;
        }
        let reach = profile.bracing.max_spacing_mm;
        // No further than a branch reaches: a trunk carried across a cavity to meet a
        // brace is a longer lever than the one the brace was to steady.
        let furthest = profile.branching.max_merge_distance_mm / 2.0;
        let here = at(tree);
        let nearest = trees
            .iter()
            .enumerate()
            .filter(|(other, _)| *other != index)
            .map(|(_, tree)| tree)
            .chain(settled)
            .filter(|tree| braceable(tree, profiles.of(tree.group())))
            .map(at)
            .min_by(|a, b| (*a - here).length().total_cmp(&(*b - here).length()));
        let Some(there) = nearest.filter(|there| (*there - here).length() > reach) else {
            continue;
        };

        let step = (there - here) * (1.0 - reach * GATHERED / (there - here).length());
        if step.length() > furthest {
            continue;
        }
        let root = *tree.root();
        let knee = root.position + step.extend(-step.length() / lean_per_drop(profile));
        let lean = Beam {
            from: root.position,
            to: knee,
            from_radius_mm: root.radius_mm,
            to_radius_mm: root.radius_mm,
        }
        .with_clearance(profile.clearance_mm);
        let stands = landing(placed, knee, root.radius_mm, tree.tip_count(), profile)
            .filter(|landing| !landing.on_model && clear(placed, &lean));
        let Some(landing) = stands else {
            continue;
        };

        let mut leaning = tree.clone();
        leaning.lean_to(knee, root.radius_mm, landing);
        if braceable(&leaning, profile) {
            trees[index] = leaning;
        }
    }
}

/// A tip or a trunk that has not been merged into anything yet.
#[derive(Debug, Clone, Copy)]
struct Front {
    position: Vec3,
    radius_mm: Scalar,
    /// How many tips hang off it, which is what its trunk's base is sized for.
    tips: usize,
    /// The group of the heaviest head it carries, which what is under it is built to.
    group: u16,
    node: usize,
    alive: bool,
}

/// Every node placed so far, and which of them are still looking for a partner.
struct Forest<'a> {
    profiles: Profiles<'a>,
    nodes: Vec<TreeNode>,
    fronts: Vec<Front>,
    /// What thicker groups have stood, for a front to run into.
    carriers: &'a Carriers,
    /// The fronts that ran into a carrier, each with where: trees with no trunk of their
    /// own.
    landed: Vec<(usize, Landing)>,
    /// Tips landed on each tree of `carriers` so far, on top of its own.
    carried: HashMap<usize, usize>,
    /// Fronts bucketed by plate square so a new one asks nine buckets; dead fronts stay and are
    /// skipped, so entries never exceed twice the number of tips.
    grid: Buckets<usize>,
}

impl<'a> Forest<'a> {
    /// One front per column, each at its contact. Every tip names its own group until
    /// the trees are cut, which is when the names their trunk took are forgotten.
    fn seeded(columns: &[Column], profiles: Profiles<'a>, carriers: &'a Carriers) -> Self {
        let cell_mm = columns
            .iter()
            .map(|column| profiles.of(column.group).branching.max_merge_distance_mm)
            .fold(SAME_PLACE_MM, Scalar::max);
        let mut forest = Self {
            profiles,
            nodes: Vec::with_capacity(columns.len()),
            fronts: Vec::with_capacity(columns.len()),
            carriers,
            landed: Vec::new(),
            carried: HashMap::new(),
            grid: Buckets::new(cell_mm),
        };

        // A tip leaves the surface square on, so the contact and the neck under it are
        // two nodes, and what merges is the neck; see `docs/decisions/0125`.
        for column in columns {
            let radius_mm = profiles.of(column.group).pillar_radius_mm();
            let headed = has_head(column.contact, column.neck);
            let contact = forest.nodes.len();
            forest.nodes.push(TreeNode {
                position: column.contact,
                radius_mm,
                parent: headed.then_some(contact + 1),
                point: Some(column.point),
                head: Some(column.group),
            });
            if headed {
                forest.nodes.push(TreeNode {
                    position: column.neck,
                    radius_mm,
                    parent: None,
                    point: None,
                    head: None,
                });
            }
            let node = forest.nodes.len() - 1;
            forest.push_front(Front {
                position: column.neck,
                radius_mm,
                tips: 1,
                group: column.group,
                node,
                alive: true,
            });
        }
        forest
    }

    /// The profile what is under `front` is built to.
    fn profile(&self, front: usize) -> &'a SupportProfile {
        self.profiles.of(self.fronts[front].group)
    }

    fn push_front(&mut self, front: Front) -> usize {
        let index = self.fronts.len();
        self.grid.insert(front.position.truncate(), index);
        self.fronts.push(front);
        index
    }

    /// Every live front in the nine buckets around `index`, barring `index` itself.
    fn neighbours(&self, index: usize) -> Vec<usize> {
        self.grid
            .around(self.fronts[index].position.truncate())
            .copied()
            .filter(|&other| other != index && self.fronts[other].alive)
            .collect()
    }
}

/// Ground a leaning trunk (a knee or a strut into a thicker support) covers per mm it descends:
/// the profile's branch angle, never past 45 degrees (ADR 0249).
fn lean_per_drop(profile: &SupportProfile) -> Scalar {
    profile.branching.reach_per_drop().min(MAX_LEAN_PER_DROP)
}

#[cfg(test)]
mod tests;
