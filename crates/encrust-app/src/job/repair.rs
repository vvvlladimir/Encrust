use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use core_geometry::{Bvh, Mended, Mesh, Scalar, self_intersections};
use core_volume::{Cancel, Progress, RepairMethod, RepairSettings, repair};

use crate::i18n::tr;
use crate::job::{Counted, Worker, pooled};
use crate::scene::{ObjectId, Shaped};

/// One model to repair, owned so that it can cross to the worker thread.
pub struct RepairRequest {
    pub id: ObjectId,
    pub name: String,
    pub mesh: Arc<Mesh>,
    pub method: RepairMethod,
    pub settings: RepairSettings,
}

/// One model as repair left it, in the space it was in.
#[derive(Debug, Clone, PartialEq)]
pub struct Repaired {
    pub id: ObjectId,
    pub name: String,
    pub shaped: Shaped,
    pub mended: Mended,
    pub method: RepairMethod,
    /// The lattice a wrap was sampled on, mm; `None` where it was cut exactly or not wrapped.
    pub voxel_mm: Option<Scalar>,
    /// Whether the mesh is the wrap's surface rather than the repair's alone.
    pub wrapped: bool,
}

/// How one repair ended.
#[derive(Debug, Clone, PartialEq)]
pub enum RepairOutcome {
    /// Boxed because it carries a whole model, like an import's own outcome.
    Done(Box<Repaired>),
    /// The whole error chain flattened into one line, as the window has no terminal.
    Failed(String),
}

/// One model being repaired on its own thread, as a wrap samples a field and any repair
/// rebuilds the hierarchy (`docs/decisions/0038`). Dropping the job asks the run to stop.
#[derive(Debug)]
pub struct RepairJob {
    worker: Worker<RepairOutcome>,
    id: ObjectId,
    name: String,
}

impl RepairJob {
    /// Starts the repair. The thread is detached: it ends on its own.
    pub fn spawn(request: RepairRequest) -> Self {
        let (id, name) = (request.id, request.name.clone());
        let worker = Worker::spawn(
            move |stop, progress| pooled(|| run(request, (stop, progress)), RepairOutcome::Failed),
            || RepairOutcome::Failed(tr!("job-thread-stopped-repair")),
        );
        Self { worker, id, name }
    }

    /// Returns the outcome on the frame the repair ends, and `None` while it is going.
    pub fn poll(&mut self) -> Option<RepairOutcome> {
        self.worker.poll()
    }

    /// How far a wrap has got; a repair on the mesh alone counts nothing.
    pub fn counted(&self) -> Option<Counted> {
        self.worker.counted()
    }

    pub fn label(&self) -> String {
        tr!("job-repairing", name = self.name.clone())
    }

    /// The model being repaired.
    pub fn id(&self) -> ObjectId {
        self.id
    }
}

fn run(request: RepairRequest, (stop, progress): (&AtomicBool, &Progress)) -> RepairOutcome {
    let asked = || stop.load(Ordering::Relaxed);
    let cancel = Cancel::when(&asked).reporting(progress);
    match repair(&request.mesh, request.method, request.settings, cancel) {
        Ok(done) => RepairOutcome::Done(Box::new(Repaired {
            id: request.id,
            name: request.name,
            shaped: Shaped::of(Arc::new(done.mesh)),
            mended: done.mended,
            method: request.method,
            voxel_mm: done.voxel_mm,
            wrapped: done.wrapped,
        })),
        Err(error) => RepairOutcome::Failed(tr!(
            "repair-failed",
            name = request.name,
            reason = error.to_string()
        )),
    }
}

/// The pairs of faces of one model that cross each other, counted on their own thread:
/// on a model of millions of faces that is longer than a frame.
#[derive(Debug)]
pub struct CrossingJob {
    worker: Worker<Option<usize>>,
}

impl CrossingJob {
    /// Starts counting over the model's own hierarchy.
    pub fn spawn(mesh: Arc<Mesh>, bvh: Arc<Bvh>) -> Self {
        let worker = Worker::spawn(
            move |_, _| Some(self_intersections(&mesh, &bvh).len()),
            || None,
        );
        Self { worker }
    }

    /// The count on the frame it is done, `Some(None)` where the thread stopped without one,
    /// and `None` while it is going.
    pub fn poll(&mut self) -> Option<Option<usize>> {
        self.worker.poll()
    }
}

#[cfg(test)]
mod tests;
