use core_geometry::{Mat4, Scalar, Vec2, Vec3};

use serde::{Deserialize, Serialize};

use crate::Landing;

/// One joint of a support, in plate coordinates.
///
/// A node hangs from the node below it, so the parent link always points downwards and
/// the tree has its root at the bottom.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TreeNode {
    pub position: Vec3,
    /// Radius of the strut arriving here, millimetres.
    pub radius_mm: Scalar,
    /// The node underneath, or `None` at the root.
    pub parent: Option<usize>,
    /// Index of the `SupportPoint` this node is the contact of, when it is one.
    pub point: Option<usize>,
}

impl TreeNode {
    pub fn is_leaf(&self) -> bool {
        self.point.is_some()
    }
}

/// One support: the tips it holds, the struts that carry them down, and where the trunk
/// underneath them comes to rest.
///
/// A support with no branching is a tree of one node, which is the vertical column of
/// step 7a. See `docs/decisions/0040` and `docs/design/supports.md`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SupportTree {
    nodes: Vec<TreeNode>,
    root: usize,
    landing: Landing,
    /// Which group's profile this tree is built to; see [`crate::Profiles`].
    group: u16,
}

impl SupportTree {
    /// `root` indexes `nodes`, and every other node reaches it through its parents.
    pub(crate) fn new(nodes: Vec<TreeNode>, root: usize, landing: Landing) -> Self {
        Self {
            nodes,
            root,
            landing,
            group: 0,
        }
    }

    pub fn set_group(&mut self, group: u16) {
        self.group = group;
    }

    /// Which group's profile the tree is built to.
    pub fn group(&self) -> u16 {
        self.group
    }

    /// Bends the tree onto a knee: a new root at `position` that the old one hangs from,
    /// for a support that had to lean before it found room to stand.
    pub(crate) fn stand_on_knee(&mut self, position: Vec3, radius_mm: Scalar) {
        let knee = self.nodes.len();
        self.nodes.push(TreeNode {
            position,
            radius_mm,
            parent: None,
            point: None,
        });
        self.nodes[self.root].parent = Some(knee);
        self.root = knee;
    }

    /// Bends the tree onto a knee at `position` and stands it on `landing` from there.
    pub(crate) fn lean_to(&mut self, position: Vec3, radius_mm: Scalar, landing: Landing) {
        self.stand_on_knee(position, radius_mm);
        self.landing = landing;
    }

    pub fn nodes(&self) -> &[TreeNode] {
        &self.nodes
    }

    /// The node the trunk descends from, the one every other node hangs off.
    pub fn root(&self) -> &TreeNode {
        &self.nodes[self.root]
    }

    pub fn root_index(&self) -> usize {
        self.root
    }

    /// Where the trunk under the root comes to rest.
    pub fn landing(&self) -> Landing {
        self.landing
    }

    /// The support points this tree holds up, in the order its nodes are stored.
    pub fn points(&self) -> impl Iterator<Item = usize> + '_ {
        self.nodes.iter().filter_map(|node| node.point)
    }

    /// How many tips the tree carries. One for a plain vertical column.
    pub fn tip_count(&self) -> usize {
        self.nodes.iter().filter(|node| node.is_leaf()).count()
    }

    /// The same tree with every position put through `matrix`, which is what carries a
    /// tree the hand has edited between the model's own space and the plate.
    ///
    /// Radii are left alone: a support keeps the thickness its profile asks for however
    /// the model it stands under is scaled.
    #[must_use]
    pub fn moved(&self, matrix: Mat4) -> Self {
        let mut moved = self.clone();
        for node in &mut moved.nodes {
            node.position = matrix.transform_point3(node.position);
        }
        moved.landing.base = matrix.transform_point3(moved.landing.base);
        moved.landing.pad_offset = matrix
            .transform_vector3(moved.landing.pad_offset.extend(0.0))
            .truncate();
        moved
    }

    /// Carries one node to `to`, which moves the struts meeting there with it: this is
    /// what bends a joint rather than lengthening a strut.
    pub fn move_node(&mut self, node: usize, to: Vec3) {
        if let Some(node) = self.nodes.get_mut(node) {
            node.position = to;
        }
    }

    /// Puts the foot at `base`, which always stands on the plate once a hand has aimed it.
    pub fn move_landing(&mut self, base: Vec3) {
        self.landing = Landing {
            base,
            on_model: false,
            pad_offset: Vec2::ZERO,
        };
    }

    /// Stands the tree on `landing`, for a trunk a hand has carried somewhere a column
    /// was dropped from again.
    pub(crate) fn stand_on(&mut self, landing: Landing) {
        self.landing = landing;
    }

    /// The tree without the nodes in `cut` and everything they hold up, or `None` when
    /// that leaves it nothing to hold: the root went, or every tip did.
    ///
    /// A joint left with nothing above it goes too, and so on down, so no stub is left
    /// reaching for a tip that is not there.
    #[must_use]
    pub fn without(&self, cut: &[usize]) -> Option<Self> {
        let mut kept: Vec<bool> = (0..self.nodes.len())
            .map(|node| !self.hangs_from(node, cut))
            .collect();
        loop {
            let mut holds = vec![false; self.nodes.len()];
            for (node, _) in self
                .nodes
                .iter()
                .enumerate()
                .filter(|(node, _)| kept[*node])
            {
                if let Some(parent) = self.nodes[node].parent {
                    holds[parent] = true;
                }
            }
            let bare: Vec<usize> = (0..self.nodes.len())
                .filter(|node| kept[*node] && !self.nodes[*node].is_leaf() && !holds[*node])
                .collect();
            if bare.is_empty() {
                break;
            }
            for node in bare {
                kept[node] = false;
            }
        }
        if !kept[self.root] {
            return None;
        }

        let mut renumbered = vec![0usize; self.nodes.len()];
        let mut next = 0;
        for (node, kept) in kept.iter().enumerate() {
            renumbered[node] = next;
            next += usize::from(*kept);
        }
        let nodes = self
            .nodes
            .iter()
            .enumerate()
            .filter(|(node, _)| kept[*node])
            .map(|(_, node)| TreeNode {
                parent: node.parent.map(|parent| renumbered[parent]),
                ..*node
            })
            .collect();
        Some(Self {
            nodes,
            root: renumbered[self.root],
            ..self.clone()
        })
    }

    /// Whether `node` is one of `cut` or hangs, through its parents, from one of them.
    fn hangs_from(&self, node: usize, cut: &[usize]) -> bool {
        let mut at = Some(node);
        // No longer than the tree is deep; the count guards a file with a loop in it.
        for _ in 0..=self.nodes.len() {
            let Some(here) = at else {
                return false;
            };
            if cut.contains(&here) {
                return true;
            }
            at = self.nodes.get(here).and_then(|node| node.parent);
        }
        false
    }

    /// Where the tree touches the model: the position of every tip it carries.
    pub fn contacts(&self) -> impl Iterator<Item = Vec3> + '_ {
        self.nodes
            .iter()
            .filter(|node| node.is_leaf())
            .map(|node| node.position)
    }

    /// Every strut, as the pair of nodes it runs between, child first.
    pub fn struts(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.nodes
            .iter()
            .enumerate()
            .filter_map(|(child, node)| node.parent.map(|parent| (child, parent)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{profile, trees_on};
    use crate::{Profiles, SupportPoint};
    use core_geometry::{Mesh, Transform};

    /// Two tips close enough to share a trunk and a third further off that joins them
    /// lower down, all 20 mm over the plate.
    fn branched() -> SupportTree {
        let points: Vec<SupportPoint> = [10.0, 13.0, 19.0]
            .into_iter()
            .map(|x| SupportPoint::new(Vec3::new(x, 10.0, 20.0)))
            .collect();
        let mut trees = trees_on(
            &points,
            &Mesh::default(),
            Transform::default(),
            Profiles::single(&profile()),
        );
        assert_eq!(trees.len(), 1, "the three tips share a trunk");
        trees.remove(0)
    }

    /// The node a tip hangs its head on: the one under the leaf holding `point`.
    fn neck_under(tree: &SupportTree, point: usize) -> usize {
        tree.nodes()
            .iter()
            .find(|node| node.point == Some(point))
            .and_then(|node| node.parent)
            .expect("a tip of a branched tree hangs from a neck")
    }

    #[test]
    fn cutting_a_strut_takes_what_it_holds_and_leaves_the_rest_standing() {
        let tree = branched();
        let left = tree
            .without(&[neck_under(&tree, 2)])
            .expect("two tips are still held");

        assert_eq!(left.tip_count(), 2);
        assert_eq!(left.points().collect::<Vec<_>>(), vec![0, 1]);
        assert_eq!(
            left.landing(),
            tree.landing(),
            "the trunk stands where it stood"
        );
        for (child, parent) in left.struts() {
            assert!(
                child < left.nodes().len() && parent < left.nodes().len(),
                "every strut still runs between two nodes of the tree"
            );
        }
        assert!(
            left.nodes()
                .iter()
                .enumerate()
                .all(|(at, node)| node.is_leaf() || left.struts().any(|(_, parent)| parent == at)),
            "no joint is left holding nothing"
        );
    }

    #[test]
    fn cutting_the_joint_two_tips_meet_at_takes_both() {
        let tree = branched();
        let joint = tree.nodes()[neck_under(&tree, 0)]
            .parent
            .expect("the neck hangs from the joint the pair meets at");
        let left = tree.without(&[joint]).expect("the third tip is still held");
        assert_eq!(left.points().collect::<Vec<_>>(), vec![2]);
    }

    #[test]
    fn cutting_every_tip_leaves_no_support() {
        let tree = branched();
        let necks: Vec<usize> = (0..3).map(|point| neck_under(&tree, point)).collect();
        assert!(tree.without(&necks).is_none());
    }

    #[test]
    fn cutting_the_root_leaves_no_support() {
        let tree = branched();
        assert!(tree.without(&[tree.root_index()]).is_none());
    }
}
