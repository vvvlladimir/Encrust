use core_csg::CsgError;
use core_geometry::{Mesh, Vec3};
use core_volume::{Boolean, Cancel, Combined, VolumeError, combine, combine_exact_toward};

use crate::i18n::tr;
use crate::job::{Worker, pooled};
use crate::scene::Taken;

/// The models a combination takes, already placed on the plate, the first being what a
/// difference keeps.
pub struct BooleanRequest {
    pub taken: Vec<Taken>,
    pub placed: Vec<Mesh>,
    pub op: Boolean,
    pub method: Method,
}

/// How the models are combined.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Method {
    /// Exactly where every model is a solid, and through the field where one is not.
    Sampled,
    /// Exactly, refusing a model the exact cut cannot read, and taking only the side of the
    /// first model the others stand on it from when they are small beside it.
    ExactToward(Vec3),
}

/// How a combination ended.
#[derive(Debug, Clone)]
pub enum BooleanOutcome {
    Combined(Combined),
    /// The whole error chain flattened into one line, as the window has no terminal.
    Failed(String),
}

/// A combination on its own thread, an exact cut of a real model being a second or more,
/// with the models it took.
#[derive(Debug)]
pub struct BooleanJob {
    worker: Worker<BooleanOutcome>,
    taken: Vec<Taken>,
}

impl BooleanJob {
    /// Starts the run. The thread is detached: it ends on its own.
    pub fn spawn(request: BooleanRequest) -> Self {
        let BooleanRequest {
            taken,
            placed,
            op,
            method,
        } = request;
        let worker = Worker::spawn(
            move || pooled(|| run(&placed, op, method), BooleanOutcome::Failed),
            || BooleanOutcome::Failed(tr!("job-thread-stopped-boolean")),
        );
        Self { worker, taken }
    }

    /// Returns the outcome on the frame the run ends, and `None` while it is going.
    pub fn poll(&mut self) -> Option<BooleanOutcome> {
        self.worker.poll()
    }

    /// The models the run took, in the order they were picked.
    pub fn taken(&self) -> &[Taken] {
        &self.taken
    }
}

fn run(placed: &[Mesh], op: Boolean, method: Method) -> BooleanOutcome {
    let combined = match method {
        Method::Sampled => combine(placed, op, Cancel::never()),
        Method::ExactToward(toward) => {
            combine_exact_toward(placed, op, toward).map(|mesh| Combined {
                mesh,
                voxel_mm: None,
            })
        }
    };
    match combined {
        Ok(combined) => BooleanOutcome::Combined(combined),
        Err(VolumeError::NothingLeft) => BooleanOutcome::Failed(tr!("boolean-nothing-left")),
        Err(VolumeError::Csg(CsgError::NotSolid(_))) => {
            BooleanOutcome::Failed(tr!("boolean-not-solid"))
        }
        Err(error) => BooleanOutcome::Failed(tr!("boolean-failed", reason = error.to_string())),
    }
}
