use std::cmp::Ordering;
use std::collections::BinaryHeap;

use core_geometry::{Scalar, Vec3};
use printer_profiles::SupportProfile;

use super::footing::footing_under;
use super::{Refused, fit, fits, head_mm, trunk_fit};
use crate::branch::{lean_per_drop, lower, meeting_point};
use crate::clear::{Beam, clear};
use crate::column::PLATE_Z_MM;
use crate::group::Profiles;
use crate::placed::Placed;
use crate::space::Space;
use crate::tree::{SupportTree, TreeNode};

/// Trees `picked` of `frozen` (model space) as one support: their trunks joined two at a time,
/// highest meeting point first, and one trunk from the last joint down (ADR 0445); or why not.
pub fn merged(
    frozen: &[SupportTree],
    picked: &[usize],
    placed: &Placed,
    profiles: Profiles,
) -> Result<SupportTree, Refused> {
    let space = Space::of(placed).ok_or(Refused::NoHold)?;
    let mut picked = picked.to_vec();
    picked.sort_unstable();
    picked.dedup();
    let trees = picked
        .iter()
        .map(|index| frozen.get(*index).map(|tree| space.plate_tree(tree)))
        .collect::<Option<Vec<_>>>()
        .filter(|trees| trees.len() >= 2)
        .ok_or(Refused::NoHold)?;

    let mut joining = Joining::seeded(&trees, profiles);
    let last = joining.join_all(placed)?;
    let profile = profiles.of(last.group);
    let landing = footing_under(placed, last.position, last.radius_mm, last.tips, profile)
        .ok_or(Refused::NoFooting)?;
    let mut tree = SupportTree::new(joining.nodes, last.node, landing);
    tree.settle(last.group);

    trunk_fit(&tree, placed, profile)?;
    // Only what the merge built answers for a support that already stood where it should not.
    if trees
        .iter()
        .all(|tree| fits(tree, placed, profiles.of(tree.group())))
    {
        fit(&tree, placed, profile)?;
    }
    Ok(space.model_tree(&tree))
}

/// A trunk not yet joined to the others: the root of a picked tree, or a joint.
#[derive(Debug, Clone, Copy)]
struct Front {
    position: Vec3,
    radius_mm: Scalar,
    tips: usize,
    /// The heaviest group it carries, which what is under it is built to.
    group: u16,
    node: usize,
    alive: bool,
}

/// Two fronts that could join, where, and how far both drop straight down first, mm.
#[derive(Debug, Clone, Copy)]
struct Candidate {
    meet: Vec3,
    a: usize,
    b: usize,
    drop_mm: Scalar,
}

impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> Ordering {
        self.meet
            .z
            .total_cmp(&other.meet.z)
            .then_with(|| other.a.cmp(&self.a))
            .then_with(|| other.b.cmp(&self.b))
            .then_with(|| other.drop_mm.total_cmp(&self.drop_mm))
    }
}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Candidate {}

/// The nodes of every picked tree (plate) in one list, and the trunks still to join.
struct Joining<'a> {
    profiles: Profiles<'a>,
    nodes: Vec<TreeNode>,
    fronts: Vec<Front>,
}

impl<'a> Joining<'a> {
    /// One front per tree at its root; a tip standing straight on its trunk gets a neck under
    /// its head first, so the head stays upright when the trunk leans away.
    fn seeded(trees: &[SupportTree], profiles: Profiles<'a>) -> Self {
        let mut joining = Self {
            profiles,
            nodes: Vec::new(),
            fronts: Vec::with_capacity(trees.len()),
        };
        for tree in trees {
            let offset = joining.nodes.len();
            joining
                .nodes
                .extend(tree.nodes().iter().enumerate().map(|(at, node)| TreeNode {
                    parent: node.parent.map(|parent| parent + offset),
                    head: node.is_leaf().then(|| tree.head_group(at)),
                    ..*node
                }));
            let mut node = offset + tree.root_index();
            let root = tree.root();
            if root.is_leaf() {
                let neck = root.position - Vec3::Z * head_mm(profiles.of(tree.group()));
                node = joining.push(neck, root.radius_mm);
                joining.nodes[offset + tree.root_index()].parent = Some(node);
            }
            joining.fronts.push(Front {
                position: joining.nodes[node].position,
                radius_mm: root.radius_mm,
                tips: tree.tip_count(),
                group: tree.group(),
                node,
                alive: true,
            });
        }
        joining
    }

    /// Joins every front into one, highest meeting point first; a pair whose struts the model
    /// is in the way of drops twice as far each retry, until it would meet under the plate.
    fn join_all(&mut self, placed: &Placed) -> Result<Front, Refused> {
        let mut heap = BinaryHeap::new();
        for a in 0..self.fronts.len() {
            for b in 0..a {
                self.offer(&mut heap, a, b, 0.0);
            }
        }
        while let Some(candidate) = heap.pop() {
            let (a, b) = (self.fronts[candidate.a], self.fronts[candidate.b]);
            if !a.alive || !b.alive {
                continue;
            }
            let profile = self.profile(&a, &b);
            if !(reaches(&a, &candidate, profile, placed)
                && reaches(&b, &candidate, profile, placed))
            {
                self.offer(
                    &mut heap,
                    candidate.a,
                    candidate.b,
                    lower(candidate.drop_mm),
                );
                continue;
            }
            let joined = self.join(&candidate);
            for other in (0..joined).filter(|other| self.fronts[*other].alive) {
                self.offer(&mut heap, joined, other, 0.0);
            }
        }
        let mut alive = self.fronts.iter().filter(|front| front.alive);
        match (alive.next(), alive.next()) {
            (Some(last), None) => Ok(*last),
            _ => Err(Refused::Collides),
        }
    }

    /// Offers the heap where fronts `a` and `b` meet once both dropped `drop_mm`, unless that is
    /// under the plate.
    fn offer(&self, heap: &mut BinaryHeap<Candidate>, a: usize, b: usize, drop_mm: Scalar) {
        let (one, two) = (self.fronts[a], self.fronts[b]);
        let reach = lean_per_drop(self.profile(&one, &two));
        let down = Vec3::Z * drop_mm;
        let meet = meeting_point(one.position - down, two.position - down, reach);
        if meet.z > PLATE_Z_MM {
            heap.push(Candidate {
                meet,
                a,
                b,
                drop_mm,
            });
        }
    }

    /// Hangs both fronts of `candidate` off a new joint at its meeting point, each through a
    /// node straight under it when it drops first.
    fn join(&mut self, candidate: &Candidate) -> usize {
        let (a, b) = (self.fronts[candidate.a], self.fronts[candidate.b]);
        let joint = self.push(candidate.meet, a.radius_mm.max(b.radius_mm));
        for front in [a, b] {
            let mut from = front.node;
            if candidate.drop_mm > 0.0 {
                let dropped = front.position - Vec3::Z * candidate.drop_mm;
                let knee = self.push(dropped, front.radius_mm);
                self.nodes[from].parent = Some(knee);
                from = knee;
            }
            self.nodes[from].parent = Some(joint);
        }
        self.fronts[candidate.a].alive = false;
        self.fronts[candidate.b].alive = false;
        self.fronts.push(Front {
            position: candidate.meet,
            radius_mm: a.radius_mm.max(b.radius_mm),
            tips: a.tips + b.tips,
            group: self.profiles.heavier(a.group, b.group),
            node: joint,
            alive: true,
        });
        self.fronts.len() - 1
    }

    /// The profile a joint of `a` and `b` is built to: the heavier one's.
    fn profile(&self, a: &Front, b: &Front) -> &'a SupportProfile {
        self.profiles.of(self.profiles.heavier(a.group, b.group))
    }

    /// A new node at `position` hanging from nothing yet; answers its index.
    fn push(&mut self, position: Vec3, radius_mm: Scalar) -> usize {
        self.nodes.push(TreeNode {
            position,
            radius_mm,
            parent: None,
            point: None,
            head: None,
        });
        self.nodes.len() - 1
    }
}

/// Whether `front` runs clear of the model down `candidate`'s drop and on to its meeting point.
fn reaches(
    front: &Front,
    candidate: &Candidate,
    profile: &SupportProfile,
    placed: &Placed,
) -> bool {
    let dropped = front.position - Vec3::Z * candidate.drop_mm;
    let beam = |from: Vec3, to: Vec3| {
        Beam {
            from,
            to,
            from_radius_mm: front.radius_mm,
            to_radius_mm: front.radius_mm,
        }
        .with_clearance(profile.clearance_mm)
    };
    (candidate.drop_mm <= 0.0 || clear(placed, &beam(front.position, dropped)))
        && clear(placed, &beam(dropped, candidate.meet))
}

#[cfg(test)]
mod tests;
