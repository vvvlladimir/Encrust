use std::ops::Range;
use std::sync::Arc;

use core_geometry::Mesh;
use core_volume::Drilled;

use crate::i18n::tr;
use crate::job::{Worker, pooled};

/// One model's cuts to take out of it exactly: what it prints from, its cavity's faces once
/// hollowed, and the cuts as `drill` laid them.
pub struct DrillRequest {
    pub source: Arc<Mesh>,
    pub cavity: Option<Range<usize>>,
    pub cuts: Arc<Mesh>,
}

/// The model drilled, or why the engine could not, which leaves its cuts to the winding rule.
pub type DrillOutcome = Result<Drilled, String>;

/// Cuts taken out on their own thread: the engine reads the whole model, a second or so on a
/// dense one (ADR 0322).
pub type DrillJob = Worker<DrillOutcome>;

/// Starts the cut. The thread is detached: it ends on its own.
pub fn spawn_drill(request: DrillRequest) -> DrillJob {
    Worker::spawn(
        // The engine cannot be given up on part way.
        move |_, _| pooled(|| drill(&request), Err),
        || Err(tr!("job-thread-stopped-drill")),
    )
}

fn drill(request: &DrillRequest) -> DrillOutcome {
    Drilled::of(&request.source, request.cavity.clone(), &request.cuts)
        .map_err(|error| error.to_string())
}
