use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

use core_geometry::{Bvh, Scalar};
use core_volume::{Cancel, DrainHole, ModelRead, PlugSource, Plugs, Progress, plugs};

use crate::i18n::tr;
use crate::job::{Worker, pooled};

/// The plugs one model's holes ask for, cut in the model's own space.
pub struct PlugsRequest {
    /// The model read once for its drill and its plugs.
    pub read: Arc<ModelRead>,
    pub bvh: Arc<Bvh>,
    /// The model as read round its last holes, which is most of the cost of a cut.
    pub source: Option<PlugSource>,
    pub holes: Vec<DrainHole>,
    pub wall_mm: Option<Scalar>,
    pub gap_mm: Scalar,
}

/// The model as read, worth keeping for the next cut, and the plugs cut from it or why they
/// could not be cut.
pub type PlugsOutcome = (Option<PlugSource>, Result<Plugs, String>);

/// Plugs cut on their own thread: each is the model met with its hole through the engine,
/// a second on a dense model read for the first time (ADR 0321).
pub type PlugsJob = Worker<PlugsOutcome>;

/// Starts the cut. The thread is detached: it ends on its own.
pub fn spawn_plugs(request: PlugsRequest) -> PlugsJob {
    Worker::spawn(
        // Given up between its steps: a read or a boolean under way ends first.
        move |stop, progress| pooled(|| cut(request, stop, progress), |error| (None, Err(error))),
        || (None, Err(tr!("job-thread-stopped-plugs"))),
    )
}

fn cut(request: PlugsRequest, stop: &AtomicBool, progress: &Progress) -> PlugsOutcome {
    let asked = || stop.load(Relaxed);
    let mut source = request.source;
    let made = plugs(
        &mut source,
        &request.read,
        &request.bvh,
        &request.holes,
        request.wall_mm,
        request.gap_mm,
        Cancel::when(&asked).reporting(progress),
    )
    .map_err(|error| error.to_string());
    (source, made)
}
