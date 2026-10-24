use super::ground::{BRIDGE_MAX_MM, escape, search_for_ground};
use super::{Forest, Front};
use crate::Landing;
use crate::column::{
    LAST_RESORT_SLOPE_DEG, MAX_LANDING_SLOPE_DEG, MIN_PILLAR_HEIGHT_MM, PLATE_Z_MM,
    SURFACE_EPSILON_MM, landing, landing_within,
};
use crate::mesher::is_small;
use crate::placed::Placed;
use crate::tree::{SupportTree, TreeNode};
use core_geometry::{Ray, Scalar, Vec2, Vec3};
use printer_profiles::SupportProfile;
use std::collections::HashMap;

/// Whether a trunk carrying `tips` under `at`, leaving it `radius_mm` wide, has anywhere
/// to go: straight down, or leaning aside the way a blocked front does.
pub(super) fn stands(
    at: Vec3,
    radius_mm: Scalar,
    tips: usize,
    placed: &Placed,
    profile: &SupportProfile,
) -> bool {
    let front = Front {
        position: at,
        radius_mm,
        tips,
        group: 0,
        node: 0,
        alive: true,
    };
    landing(placed, at, radius_mm, tips, profile).is_some()
        || escape(&front, placed, profile, MAX_LANDING_SLOPE_DEG).is_some()
}

impl Forest<'_> {
    /// Drops a trunk from every surviving front and splits the arena into one tree each.
    pub(super) fn harvest(self, placed: &Placed) -> Vec<SupportTree> {
        let mut children: Vec<Vec<usize>> = vec![Vec::new(); self.nodes.len()];
        for (index, node) in self.nodes.iter().enumerate() {
            if let Some(parent) = node.parent {
                children[parent].push(index);
            }
        }

        self.fronts
            .iter()
            .filter(|front| front.alive)
            .flat_map(|front| {
                let trees = self.stand(front, &children, placed);
                match &trees[..] {
                    [tree] if tree.landing().on_model => {
                        self.small_apart(front, &children, placed).unwrap_or(trees)
                    }
                    _ => trees,
                }
            })
            .chain(
                self.landed
                    .iter()
                    .map(|(front, landing)| self.cut(&self.fronts[*front], &children, *landing)),
            )
            .collect()
    }

    /// What stands under `front`: down, then aside, again onto a steeper face, then a long lean for
    /// ground, before the tips get a strut each; a cured slope support beats none.
    pub(super) fn stand(
        &self,
        front: &Front,
        children: &[Vec<usize>],
        placed: &Placed,
    ) -> Vec<SupportTree> {
        // A contact with no head has the model too near under it for one: it is the small
        // pillar onto that model it was left headless for (ADR 0331), before any lean.
        if self.nodes[front.node].is_leaf() {
            let straight = self.bridges(front, children, placed);
            if !straight.is_empty() {
                return straight;
            }
        }
        let profile = self.profiles.of(front.group);
        let steeper = [MAX_LANDING_SLOPE_DEG, LAST_RESORT_SLOPE_DEG];
        for steepest_deg in steeper {
            let (at, radius_mm, tips) = (front.position, front.radius_mm, front.tips);
            if let Some(landing) =
                landing_within(placed, at, radius_mm, tips, profile, steepest_deg)
            {
                return vec![self.cut(front, children, landing)];
            }
            if let Some(leant) = escape(front, placed, profile, steepest_deg) {
                return vec![self.on_knee(front, children, leant)];
            }
        }
        for steepest_deg in steeper {
            if let Some(leant) = search_for_ground(front, placed, profile, steepest_deg) {
                return vec![self.on_knee(front, children, leant)];
            }
        }
        self.bridges(front, children, placed)
    }

    /// The tree hanging off `front`, its trunk bent at `knee` onto `landing`.
    pub(super) fn on_knee(
        &self,
        front: &Front,
        children: &[Vec<usize>],
        (knee, landing): (Vec3, Landing),
    ) -> SupportTree {
        let mut tree = self.cut(front, children, landing);
        tree.stand_on_knee(knee, front.radius_mm);
        tree
    }

    /// The tips under a front on the model, each a small pillar from its own neck, or `None` unless
    /// all are: a joint between two faces is a full support, not two thin snapping struts.
    pub(super) fn small_apart(
        &self,
        front: &Front,
        children: &[Vec<usize>],
        placed: &Placed,
    ) -> Option<Vec<SupportTree>> {
        if front.tips < 2 {
            return None;
        }
        let mut under_it = vec![front.node];
        let mut index = 0;
        while index < under_it.len() {
            under_it.extend(children[under_it[index]].iter().copied());
            index += 1;
        }
        under_it
            .into_iter()
            .filter(|node| self.nodes[*node].is_leaf())
            .map(|tip| {
                let group = self.nodes[tip].head.unwrap_or(front.group);
                // A neck carries its tip alone; a parent carrying more is a joint, and
                // the tip under it has no head.
                let node = self.nodes[tip]
                    .parent
                    .filter(|parent| children[*parent].len() == 1)
                    .unwrap_or(tip);
                let alone = Front {
                    position: self.nodes[node].position,
                    radius_mm: self.nodes[node].radius_mm,
                    tips: 1,
                    group,
                    node,
                    alive: true,
                };
                let small = |trees: Vec<SupportTree>| match &trees[..] {
                    [tree]
                        if tree.landing().on_model && is_small(tree, self.profiles.of(group)) =>
                    {
                        Some(tree.clone())
                    }
                    _ => None,
                };
                // Where the pillar's width finds no room, the thin strut may still fit
                // straight down from the contact.
                small(self.stand(&alone, children, placed))
                    .or_else(|| small(self.bridges(&alone, children, placed)))
            })
            .collect()
    }

    /// Last resort for tips under a front with nowhere to stand: each on one strut straight down
    /// onto the model, a one-node tree meshed as a small pillar where the profile has one.
    pub(super) fn bridges(
        &self,
        front: &Front,
        children: &[Vec<usize>],
        placed: &Placed,
    ) -> Vec<SupportTree> {
        let mut under_it = vec![front.node];
        let mut index = 0;
        while index < under_it.len() {
            under_it.extend(children[under_it[index]].iter().copied());
            index += 1;
        }
        under_it
            .into_iter()
            .map(|node| self.nodes[node])
            .filter(TreeNode::is_leaf)
            .filter_map(|tip| {
                let group = tip.head.unwrap_or(front.group);
                if !self.profiles.of(group).land_on_model {
                    return None;
                }
                // From just under the contact: one placed by hand sits on the face itself.
                let ray = Ray::new(tip.position - Vec3::Z * SURFACE_EPSILON_MM, -Vec3::Z);
                let under = placed
                    .raycast(&ray)
                    // A face looking up at the contact: one looking away is the inside of
                    // the model, and a strut to it is built in the solid and pokes out.
                    .filter(|hit| hit.normal.z > 0.0)
                    .filter(|hit| hit.point.z > PLATE_Z_MM && !placed.blocks_at(hit.point))
                    .filter(|hit| (MIN_PILLAR_HEIGHT_MM..=BRIDGE_MAX_MM).contains(&hit.distance))?;
                let node = TreeNode {
                    parent: None,
                    ..tip
                };
                let landing = Landing {
                    base: under.point,
                    on_model: true,
                    pad_offset: Vec2::ZERO,
                    pad_cut: None,
                    on_support: false,
                };
                let mut tree = SupportTree::new(vec![node], 0, landing);
                tree.settle(group);
                Some(tree)
            })
            .collect()
    }

    /// The tree hanging off `front`, with its nodes renumbered from zero, built to the
    /// heaviest head it carries.
    pub(super) fn cut(
        &self,
        front: &Front,
        children: &[Vec<usize>],
        landing: Landing,
    ) -> SupportTree {
        let mut order = vec![front.node];
        let mut index = 0;
        while index < order.len() {
            order.extend(children[order[index]].iter().copied());
            index += 1;
        }

        let mut local = HashMap::with_capacity(order.len());
        for (position, &node) in order.iter().enumerate() {
            local.insert(node, position);
        }
        let nodes = order
            .iter()
            .map(|&node| TreeNode {
                parent: self.nodes[node].parent.and_then(|parent| {
                    // The root's own parent is outside this tree, which is how it became
                    // a root in the first place.
                    local.get(&parent).copied()
                }),
                ..self.nodes[node]
            })
            .collect();

        let mut tree = SupportTree::new(nodes, 0, landing);
        tree.settle(front.group);
        tree
    }
}
