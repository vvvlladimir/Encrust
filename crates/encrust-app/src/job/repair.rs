use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use core_geometry::{
    Filled, Mesh, MeshDiagnostics, Orientation, collapse_flat_faces, diagnose, fill_holes,
    orient_outward, remove_duplicate_faces,
};

use crate::i18n::tr;
use crate::scene::{ObjectId, Shaped};

/// One model to close the holes in, owned so that it can cross to the worker thread.
pub struct RepairRequest {
    pub id: ObjectId,
    pub name: String,
    pub mesh: Arc<Mesh>,
}

/// One model as repair left it, in the space it was in.
#[derive(Debug, Clone, PartialEq)]
pub struct Repaired {
    pub id: ObjectId,
    pub name: String,
    pub shaped: Shaped,
    pub filled: Filled,
    /// Faces that were lying over another face and have gone.
    pub duplicates_removed: usize,
    /// What the patch had to be turned round, since a new face can come out either way.
    pub orientation: Orientation,
    /// The repair would have left more wrong than it found, so the mesh is as it came.
    pub kept_as_found: bool,
}

/// How one repair ended.
#[derive(Debug, Clone, PartialEq)]
pub enum RepairOutcome {
    /// Boxed because it carries a whole model, like an import's own outcome.
    Done(Box<Repaired>),
    /// The whole error chain flattened into one line, as the window has no terminal.
    Failed(String),
}

/// One model being repaired on its own thread.
///
/// Closing the holes walks the mesh's edges and rebuilds its hierarchy afterwards, which
/// is far too much for a frame on a real part; see `docs/decisions/0038`.
#[derive(Debug)]
pub struct RepairJob {
    outcome: Receiver<RepairOutcome>,
    id: ObjectId,
    name: String,
}

impl RepairJob {
    /// Starts the repair. The thread is detached: it ends on its own.
    pub fn spawn(request: RepairRequest) -> Self {
        let (sender, outcome) = mpsc::channel();
        let (id, name) = (request.id, request.name.clone());
        crate::job::spawn(move || {
            let _ = sender.send(run(request));
        });
        Self { outcome, id, name }
    }

    /// Returns the outcome on the frame the repair ends, and `None` while it is going.
    pub fn poll(&mut self) -> Option<RepairOutcome> {
        match self.outcome.try_recv() {
            Ok(outcome) => Some(outcome),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(RepairOutcome::Failed(tr!(
                "job-thread-stopped-repair",
                name = self.name.clone()
            ))),
        }
    }

    pub fn label(&self) -> String {
        tr!("job-repairing", name = self.name.clone())
    }

    /// The model being repaired.
    pub fn id(&self) -> ObjectId {
        self.id
    }
}

fn run(request: RepairRequest) -> RepairOutcome {
    let mut mesh = (*request.mesh).clone();
    let mended = mend(&mut mesh);
    RepairOutcome::Done(Box::new(Repaired {
        id: request.id,
        name: request.name,
        shaped: Shaped::of(Arc::new(mesh)),
        filled: mended.filled,
        duplicates_removed: mended.duplicates_removed,
        orientation: mended.orientation,
        kept_as_found: mended.kept_as_found,
    }))
}

/// What mending one mesh changed.
#[derive(Debug, Clone, PartialEq)]
pub struct Mended {
    pub filled: Filled,
    pub duplicates_removed: usize,
    pub orientation: Orientation,
    pub kept_as_found: bool,
}

/// Collapses the faces with no area, patches the holes and drops the faces laid over
/// others, and turns the patches the right way out; leaves the mesh as it came if that
/// would leave more wrong with it, or nothing of it at all.
///
/// A face with no area goes first, by welding the corners it has on one point. The doubled
/// faces go next, since one reads as a branch the boundary walk cannot get
/// through, and again after the filling: a patch over a lone face or a fin is its reverse,
/// and the two cancel. The orientation pass is last because a patch is new surface and
/// nothing has decided its side yet. See `docs/design/mesh-repair.md`.
pub fn mend(mesh: &mut Mesh) -> Mended {
    let found = mesh.clone();
    let before = wrong(&diagnose(mesh));

    collapse_flat_faces(mesh);
    let mut duplicates_removed = remove_duplicate_faces(mesh).len();
    orient_outward(mesh);
    let filled = fill_holes(mesh);
    duplicates_removed += remove_duplicate_faces(mesh).len();
    let orientation = orient_outward(mesh);

    // A sheet closed by its own reverse cancels away; a model that is nothing else would
    // vanish from the plate rather than be repaired.
    let kept_as_found = mesh.faces.is_empty() || wrong(&diagnose(mesh)) > before;
    if kept_as_found {
        *mesh = found;
    }
    Mended {
        filled,
        duplicates_removed,
        orientation,
        kept_as_found,
    }
}

/// Everything that keeps a mesh from being sound, counted together.
fn wrong(diagnostics: &MeshDiagnostics) -> usize {
    diagnostics.boundary_edges + diagnostics.degenerate_faces + diagnostics.duplicate_faces
}

#[cfg(test)]
mod tests;
