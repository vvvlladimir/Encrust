use std::sync::mpsc::{self, Receiver, TryRecvError};

use core_geometry::{Mesh, Scalar};
use core_volume::{Boolean, Cancel, VolumeError, combine};

use crate::i18n::tr;
use crate::job::pipeline::{thread_pool, worker_threads};
use crate::scene::ObjectId;

/// The models a combination takes, already placed on the plate, the first being what a
/// difference keeps.
pub struct BooleanRequest {
    pub ids: Vec<ObjectId>,
    pub name: String,
    pub placed: Vec<Mesh>,
    pub op: Boolean,
}

/// How a combination ended.
#[derive(Debug, Clone, PartialEq)]
pub enum BooleanOutcome {
    Combined {
        ids: Vec<ObjectId>,
        name: String,
        mesh: Mesh,
        /// The lattice it was sampled on, millimetres, when it could not be cut exactly.
        voxel_mm: Option<Scalar>,
    },
    /// The whole error chain flattened into one line, as the window has no terminal.
    Failed(String),
}

/// A combination on its own thread: an exact cut of a real model is a second or more.
#[derive(Debug)]
pub struct BooleanJob {
    outcome: Receiver<BooleanOutcome>,
}

impl BooleanJob {
    /// Starts the run. The thread is detached: it ends on its own.
    pub fn spawn(request: BooleanRequest) -> Self {
        let (sender, outcome) = mpsc::channel();
        crate::job::spawn(move || {
            let _ = sender.send(run(request));
        });
        Self { outcome }
    }

    /// Returns the outcome on the frame the run ends, and `None` while it is going.
    pub fn poll(&mut self) -> Option<BooleanOutcome> {
        match self.outcome.try_recv() {
            Ok(outcome) => Some(outcome),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                Some(BooleanOutcome::Failed(tr!("job-thread-stopped-boolean")))
            }
        }
    }
}

fn run(request: BooleanRequest) -> BooleanOutcome {
    let pool = match thread_pool(worker_threads()) {
        Ok(pool) => pool,
        Err(error) => return BooleanOutcome::Failed(error.to_string()),
    };
    match pool.install(|| combine(&request.placed, request.op, Cancel::never())) {
        Ok(combined) => BooleanOutcome::Combined {
            ids: request.ids,
            name: request.name,
            mesh: combined.mesh,
            voxel_mm: combined.voxel_mm,
        },
        Err(VolumeError::NothingLeft) => BooleanOutcome::Failed(tr!("boolean-nothing-left")),
        Err(error) => BooleanOutcome::Failed(tr!("boolean-failed", reason = error.to_string())),
    }
}
