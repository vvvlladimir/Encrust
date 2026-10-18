use std::sync::mpsc::{self, Receiver, TryRecvError};

use core_geometry::{Mesh, Vec3};
use core_volume::{Cancel, PartSettings, PartedOff, Parts, VolumeError, part_off, parts};

use crate::i18n::tr;
use crate::job::pipeline::{thread_pool, worker_threads};
use crate::scene::ObjectId;

/// One model to split into its parts, already placed on the plate.
pub struct SplitRequest {
    pub id: ObjectId,
    pub placed: Mesh,
    pub settings: PartSettings,
    /// The point on the plate a part is marked at, to cut that part off instead.
    pub at: Option<Vec3>,
}

/// How a split ended.
#[derive(Debug)]
pub enum SplitOutcome {
    Split { id: ObjectId, parts: Parts },
    PartedOff { id: ObjectId, parted: PartedOff },
    Failed(String),
}

/// A split on its own thread: cutting off a marked part builds a field of the model and
/// cuts it exactly, seconds on a real one.
#[derive(Debug)]
pub struct SplitJob {
    outcome: Receiver<SplitOutcome>,
}

impl SplitJob {
    /// Starts the run. The thread is detached: it ends on its own.
    pub fn spawn(request: SplitRequest) -> Self {
        let (sender, outcome) = mpsc::channel();
        crate::job::spawn(move || {
            let _ = sender.send(run(request));
        });
        Self { outcome }
    }

    /// Returns the outcome on the frame the run ends, and `None` while it is going.
    pub fn poll(&mut self) -> Option<SplitOutcome> {
        match self.outcome.try_recv() {
            Ok(outcome) => Some(outcome),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                Some(SplitOutcome::Failed(tr!("job-thread-stopped-split")))
            }
        }
    }
}

fn run(request: SplitRequest) -> SplitOutcome {
    let pool = match thread_pool(worker_threads()) {
        Ok(pool) => pool,
        Err(error) => return SplitOutcome::Failed(error.to_string()),
    };
    let Some(at) = request.at else {
        return SplitOutcome::Split {
            id: request.id,
            parts: pool.install(|| parts(&request.placed, &request.settings)),
        };
    };
    match pool.install(|| part_off(&request.placed, at, Cancel::never())) {
        Ok(parted) => SplitOutcome::PartedOff {
            id: request.id,
            parted,
        },
        Err(VolumeError::NoNeck | VolumeError::NotOnModel) => {
            SplitOutcome::Failed(tr!("cut-no-neck"))
        }
        Err(error) => SplitOutcome::Failed(tr!("cut-split-failed", reason = error.to_string())),
    }
}
