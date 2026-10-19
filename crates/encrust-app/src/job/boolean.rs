use std::sync::mpsc::{self, Receiver, TryRecvError};

use core_geometry::{Mesh, Scalar, Vec3};
use core_volume::{
    Boolean, Cancel, Combined, VolumeError, combine, combine_exact, combine_exact_toward,
};

use crate::i18n::tr;
use crate::job::pipeline::{thread_pool, worker_threads};
use crate::scene::Taken;

/// The models a combination takes, already placed on the plate, the first being what a
/// difference keeps.
pub struct BooleanRequest {
    pub taken: Vec<Taken>,
    pub name: String,
    pub placed: Vec<Mesh>,
    pub op: Boolean,
    /// Refuse rather than sample a model the exact cut cannot read.
    pub exact: bool,
    /// The side the others stand on the first from, when they are small beside it: an exact
    /// cut then takes only that side of it through the engine.
    pub toward: Option<Vec3>,
}

/// How a combination ended.
#[derive(Debug, Clone)]
pub enum BooleanOutcome {
    Combined {
        taken: Vec<Taken>,
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
    let combined = pool.install(|| {
        if request.exact {
            match request.toward {
                Some(toward) => combine_exact_toward(&request.placed, request.op, toward),
                None => combine_exact(&request.placed, request.op),
            }
            .map(|mesh| Combined {
                mesh,
                voxel_mm: None,
            })
        } else {
            combine(&request.placed, request.op, Cancel::never())
        }
    });
    match combined {
        Ok(combined) => BooleanOutcome::Combined {
            taken: request.taken,
            name: request.name,
            mesh: combined.mesh,
            voxel_mm: combined.voxel_mm,
        },
        Err(VolumeError::NothingLeft) => BooleanOutcome::Failed(tr!("boolean-nothing-left")),
        Err(VolumeError::NotSolid(_)) => BooleanOutcome::Failed(tr!("boolean-not-solid")),
        Err(error) => BooleanOutcome::Failed(tr!("boolean-failed", reason = error.to_string())),
    }
}
