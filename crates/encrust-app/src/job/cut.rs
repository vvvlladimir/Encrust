use std::sync::Arc;

use core_engine::project::Keep;
use core_geometry::{Mesh, Plane, Transform, transform_mesh};
use core_joint::{Joined, Joints, cut_and_join};
use core_volume::drilled;

use crate::i18n::tr;
use crate::job::{Worker, pooled};

/// One model to cut, in its own space and where it stands on the plate.
pub struct CutRequest {
    /// What prints: the model or its hollowed shell, its cuts taken out where they have been.
    pub printed: Arc<Mesh>,
    /// The holes and channels still to be taken out of `printed` before it is cut.
    pub cuts: Option<Arc<Mesh>>,
    /// The model's outside, where what prints is its shell.
    pub solid: Option<Arc<Mesh>>,
    pub transform: Transform,
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
    /// The model's holes cannot be taken out exactly, so halves cut from it would lose them.
    Undrilled,
    Failed(String),
}

/// A cut on its own thread: building pins into a real model is a boolean of a second or
/// more.
pub type CutJob = Worker<CutOutcome>;

/// Starts the cut. The thread is detached: it ends on its own.
pub fn spawn_cut(request: CutRequest) -> CutJob {
    Worker::spawn(
        // Neither the cut nor the joint can be given up on part way.
        move |_, _| pooled(|| cut(request), CutOutcome::Failed),
        || CutOutcome::Failed(tr!("job-thread-stopped-cut")),
    )
}

fn cut(request: CutRequest) -> CutOutcome {
    // The halves are new models with no holes of their own, so they are cut from the drilled one.
    let printed = match &request.cuts {
        Some(cuts) => match drilled(&request.printed, cuts) {
            Ok(mesh) => transform_mesh(&mesh, request.transform),
            Err(_) => return CutOutcome::Undrilled,
        },
        None => transform_mesh(&request.printed, request.transform),
    };
    let solid = request
        .solid
        .map(|solid| transform_mesh(&solid, request.transform));
    let joints = (request.keep == Keep::Both).then_some(&request.joints);
    // A plane that misses leaves the model whole. Replacing it with a copy of itself
    // would throw away its supports and its cavity for nothing.
    let Some(cut) = cut_and_join(&printed, solid.as_ref(), request.plane, joints) else {
        return CutOutcome::Missed;
    };
    CutOutcome::Cut {
        joined: cut.joined,
        open_loops: cut.open_loops,
        unjoined: cut.unjoined.map(|error| error.to_string()),
    }
}
