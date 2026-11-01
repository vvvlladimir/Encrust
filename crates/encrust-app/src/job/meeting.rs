use std::sync::Arc;

use core_geometry::{Bvh, Mesh, Transform};
use core_plate::{Body, BodyKind, Meeting, crossings};

use crate::job::{Worker, pooled};

/// One body the run is handed, held so the thread outlives the frame that asked.
pub struct MeetingBody {
    /// Which model of the run it is, or stands under.
    pub owner: usize,
    pub kind: BodyKind,
    pub mesh: Arc<Mesh>,
    pub bvh: Option<Arc<Bvh>>,
    pub transform: Transform,
}

/// The models that meet, by their `owner`, and where, found on a thread of their own
/// (ADR 0390).
pub type MeetingJob = Worker<Vec<Meeting>>;

/// Starts the search. A thread that fails finds nothing: the warning is only a warning.
pub fn spawn_meeting(bodies: Arc<[MeetingBody]>) -> MeetingJob {
    Worker::spawn(
        move |_, _| pooled(|| met(&bodies), |_| Vec::new()),
        Vec::new,
    )
}

fn met(bodies: &[MeetingBody]) -> Vec<Meeting> {
    let bodies: Vec<Body<'_>> = bodies
        .iter()
        .map(|body| Body {
            owner: body.owner,
            kind: body.kind,
            mesh: &body.mesh,
            bvh: body.bvh.as_deref(),
            transform: body.transform,
        })
        .collect();
    crossings(&bodies)
}
