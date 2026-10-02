use core_geometry::{Mat4, Scalar, Vec3};

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
        };
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
