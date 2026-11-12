use core_geometry::{Mat4, Transform, Vec3};

use crate::Landing;
use crate::placed::Placed;
use crate::tree::SupportTree;

/// The two spaces an edit moves between: the model's own, which a frozen tree is kept in, and
/// the plate, which a carry steps across and every check is made in.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Space {
    to_plate: Mat4,
    to_model: Mat4,
}

impl Space {
    /// The spaces `placed` stands its model in; `None` for a model flattened to nothing.
    pub(crate) fn of(placed: &Placed) -> Option<Self> {
        Self::new(placed.transform)
    }

    /// The spaces `transform` places a model in; `None` for one flattening it to nothing.
    pub(crate) fn new(transform: Transform) -> Option<Self> {
        Some(Self {
            to_plate: transform.to_matrix(),
            to_model: transform.inverse_matrix()?,
        })
    }

    pub(crate) fn plate(self, point: Vec3) -> Vec3 {
        self.to_plate.transform_point3(point)
    }

    pub(crate) fn model(self, point: Vec3) -> Vec3 {
        self.to_model.transform_point3(point)
    }

    pub(crate) fn plate_tree(self, tree: &SupportTree) -> SupportTree {
        tree.moved(self.to_plate)
    }

    pub(crate) fn model_tree(self, tree: &SupportTree) -> SupportTree {
        tree.moved(self.to_model)
    }

    pub(crate) fn plate_landing(self, landing: Landing) -> Landing {
        landing_through(landing, self.to_plate)
    }

    pub(crate) fn model_landing(self, landing: Landing) -> Landing {
        landing_through(landing, self.to_model)
    }

    /// Where node `node` of `tree` (model space) stands on the plate.
    pub(crate) fn node(self, tree: &SupportTree, node: usize) -> Vec3 {
        self.plate(tree.nodes()[node].position)
    }

    /// Puts node `node` of `tree` (model space) at `to` (plate).
    pub(crate) fn put(self, tree: &mut SupportTree, node: usize, to: Vec3) {
        tree.move_node(node, self.model(to));
    }

    /// Puts the foot of `tree` (model space) at `base` (plate).
    pub(crate) fn put_foot(self, tree: &mut SupportTree, base: Vec3) {
        tree.move_landing(self.model(base));
    }
}

/// `landing` moved through `matrix`, from one of the two spaces to the other.
fn landing_through(landing: Landing, matrix: Mat4) -> Landing {
    Landing {
        base: matrix.transform_point3(landing.base),
        pad_offset: matrix
            .transform_vector3(landing.pad_offset.extend(0.0))
            .truncate(),
        pad_cut: landing.pad_cut.map(|cut| cut.moved(matrix)),
        ..landing
    }
}
