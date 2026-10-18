use std::sync::mpsc::{self, Receiver, TryRecvError};

use core_engine::project::Keep;
use core_geometry::{Mesh, Plane, cut};
use core_joint::{Halves, Joined, Joints, join};

use crate::i18n::tr;
use crate::job::pipeline::{thread_pool, worker_threads};
use crate::scene::ObjectId;

/// One model to cut, already placed on the plate.
pub struct CutRequest {
    pub id: ObjectId,
    /// What prints: the model, or its hollowed shell.
    pub printed: Mesh,
    /// The model's outside, where what prints is its shell.
    pub solid: Option<Mesh>,
    pub plane: Plane,
    pub keep: Keep,
    pub joints: Joints,
}

/// How a cut ended.
#[derive(Debug)]
pub enum CutOutcome {
    Cut {
        id: ObjectId,
        joined: Joined,
        /// Cut loops that did not close, and were left uncapped.
        open_loops: usize,
        /// Why the halves were left plain when they were to be joined.
        unjoined: Option<String>,
    },
    /// The plane does not cross the model, which is left as it was.
    Missed,
    Failed(String),
}

/// A cut on its own thread: building pins into a real model is a boolean of a second or
/// more.
#[derive(Debug)]
pub struct CutJob {
    outcome: Receiver<CutOutcome>,
}

impl CutJob {
    /// Starts the run. The thread is detached: it ends on its own.
    pub fn spawn(request: CutRequest) -> Self {
        let (sender, outcome) = mpsc::channel();
        crate::job::spawn(move || {
            let _ = sender.send(run(request));
        });
        Self { outcome }
    }

    /// Returns the outcome on the frame the run ends, and `None` while it is going.
    pub fn poll(&mut self) -> Option<CutOutcome> {
        match self.outcome.try_recv() {
            Ok(outcome) => Some(outcome),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                Some(CutOutcome::Failed(tr!("job-thread-stopped-cut")))
            }
        }
    }
}

fn run(request: CutRequest) -> CutOutcome {
    let pool = match thread_pool(worker_threads()) {
        Ok(pool) => pool,
        Err(error) => return CutOutcome::Failed(error.to_string()),
    };
    pool.install(|| cut_and_join(request))
}

fn cut_and_join(request: CutRequest) -> CutOutcome {
    let halves = cut(&request.printed, request.plane);
    // A plane that misses leaves the model whole. Replacing it with a copy of itself
    // would throw away its supports and its cavity for nothing.
    if halves.below.is_empty() || halves.above.is_empty() {
        return CutOutcome::Missed;
    }
    let open_loops = halves.open_loops;
    let plain = |halves: core_geometry::Cut| Joined {
        below: halves.below,
        above: halves.above,
        ..Joined::default()
    };
    if request.keep != Keep::Both || request.joints.kind.is_none() {
        return CutOutcome::Cut {
            id: request.id,
            joined: plain(halves),
            open_loops,
            unjoined: None,
        };
    }
    let joining = Halves {
        cut: halves.clone(),
        plane: request.plane,
        printed: &request.printed,
        solid: request.solid.as_ref(),
    };
    let (joined, unjoined) = match join(joining, &request.joints) {
        Ok(joined) => (joined, None),
        Err(error) => (plain(halves), Some(error.to_string())),
    };
    CutOutcome::Cut {
        id: request.id,
        joined,
        open_loops,
        unjoined,
    }
}
