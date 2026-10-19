use core_engine::project::Keep;
use core_geometry::{Mesh, Plane, cut};
use core_joint::{Halves, Joined, Joints, join};

use crate::i18n::tr;
use crate::job::{Worker, pooled};

/// One model to cut, already placed on the plate.
pub struct CutRequest {
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
pub type CutJob = Worker<CutOutcome>;

/// Starts the cut. The thread is detached: it ends on its own.
pub fn spawn_cut(request: CutRequest) -> CutJob {
    Worker::spawn(
        move || pooled(|| cut_and_join(request), CutOutcome::Failed),
        || CutOutcome::Failed(tr!("job-thread-stopped-cut")),
    )
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
        joined,
        open_loops,
        unjoined,
    }
}
