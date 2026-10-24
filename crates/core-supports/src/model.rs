use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use core_geometry::{Mesh, Region, Scalar, Transform, Vec3};

use crate::{Clearances, SupportPoint, SupportTree};

mod build;
mod frozen;
mod growth;
mod paint;
mod points;
use build::Built;
use growth::Forest;
pub use growth::{Grown, Growth};
use paint::Patches;

/// The next revision any model's supports take, shared by all of them so that no two
/// states, even one put back by an undo, are ever stamped alike.
static REVISIONS: AtomicU64 = AtomicU64::new(1);

fn next_revision() -> u64 {
    REVISIONS.fetch_add(1, Ordering::Relaxed)
}

/// The supports of one model: points in model space, paint, and the trees and meshes built
/// from them (ADR 0129); trees regrow only when points, placement, profile or block change.
#[derive(Debug, Clone, Default)]
pub struct ModelSupports {
    points: Vec<SupportPoint>,
    /// The patch the user has painted to be filled with supports.
    painted: Region,
    /// The patch the user has painted to be left alone.
    blocked: Region,
    /// The supports the hand has taken hold of, kept as the trees they are, in the
    /// model's own space. Nothing regrows them; see `docs/decisions/0095`.
    frozen: Vec<SupportTree>,
    /// The two patches as geometry, apart from the columns so a stroke shows in its own frame;
    /// columns take too long to rebuild per frame.
    patches: Patches,
    /// The trees last grown from the points, which outlive `built`: growing is the costly
    /// part of a rebuild, meshing is not.
    forest: Option<Forest>,
    built: Option<Built>,
    /// The column meshes as they were last built. `built` is dropped by every edit and
    /// rebuilt a frame later, and what is drawn in between is this; see [`Self::drawn`].
    drawn: Vec<Arc<Mesh>>,
    /// How much of the height the model stands at is its raft's doing, millimetres: what
    /// has to be taken back when the raft goes. See `docs/decisions/0238`.
    raft_lift_mm: Scalar,
    /// What the trees grow from as it last changed: the points, the blocked patch or the
    /// surface. Trees grown elsewhere for another revision are stale.
    revision: u64,
    /// What the model's clearance sweeps answered, shared with every growth of it so one
    /// edit re-asks only the beams it moved.
    clearances: Arc<Clearances>,
}

impl ModelSupports {
    /// The supports a project file was saved with. Everything built from them is left
    /// out of the file and grown again on the next rebuild.
    pub fn restore(
        points: Vec<SupportPoint>,
        painted: Region,
        blocked: Region,
        frozen: Vec<SupportTree>,
    ) -> Self {
        Self {
            points,
            painted,
            blocked,
            frozen,
            ..Self::default()
        }
    }

    /// Drops what was built, so the next refresh builds against a surface that changed
    /// under the supports: a model hollowed, or solid again.
    pub fn surface_changed(&mut self) {
        self.clearances = Arc::default();
        self.points_changed();
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty() && self.frozen.is_empty()
    }

    /// How far the model has been stood up to clear its raft, millimetres.
    pub fn raft_lift_mm(&self) -> Scalar {
        self.raft_lift_mm
    }

    /// Records how far the model now stands up to clear its raft; whoever moves the
    /// model keeps the two in step.
    pub fn set_raft_lift_mm(&mut self, lift_mm: Scalar) {
        self.raft_lift_mm = lift_mm;
    }

    pub fn point_count(&self) -> usize {
        self.points.len()
            + self
                .frozen
                .iter()
                .map(SupportTree::tip_count)
                .sum::<usize>()
    }

    /// How many supports are built to `group`: its points and the tips of its frozen
    /// trees, which is what dropping that group would take away.
    pub fn group_count(&self, group: u16) -> usize {
        self.points
            .iter()
            .filter(|support| support.group == group)
            .count()
            + self
                .frozen
                .iter()
                .flat_map(SupportTree::tip_groups)
                .filter(|tip| *tip == group)
                .count()
    }

    /// Where every support touches the model, in plate coordinates: the points and the
    /// tips of the frozen trees alike. What an automatic run is handed as seeds.
    pub fn contacts(&self, transform: Transform) -> Vec<Vec3> {
        let matrix = transform.to_matrix();
        self.points
            .iter()
            .map(|point| point.contact)
            .chain(self.frozen.iter().flat_map(SupportTree::contacts))
            .map(|contact| matrix.transform_point3(contact))
            .collect()
    }

    /// Where the supports touch the model, in the model's own space.
    pub fn points(&self) -> &[SupportPoint] {
        &self.points
    }

    /// The supports as they stand: one tree per trunk, each carrying one tip or several.
    /// The grown ones come first and the frozen ones after them.
    pub fn trees(&self) -> &[SupportTree] {
        self.built.as_ref().map_or(&[], |built| &built.trees)
    }

    /// How many tips are being carried. Fewer than the points when one of them has no
    /// room under it.
    pub fn standing(&self) -> usize {
        self.trees().iter().map(SupportTree::tip_count).sum()
    }

    /// The column meshes, one per group and in plate coordinates, or `None` until they
    /// have been built.
    pub fn meshes(&self) -> Option<&[Arc<Mesh>]> {
        self.built.as_ref().map(|built| built.meshes.as_slice())
    }

    /// The column meshes to draw: the ones last built, left standing until the rebuild an edit
    /// follows with; drawing [`Self::meshes`] instead blinks every support out for a frame.
    pub fn drawn(&self) -> &[Arc<Mesh>] {
        &self.drawn
    }
}

#[cfg(test)]
mod tests;
