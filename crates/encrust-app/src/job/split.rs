use std::sync::atomic::{AtomicBool, Ordering};

use core_geometry::{Mesh, Vec3};
use core_volume::{Cancel, PartSettings, PartedOff, Parts, Progress, VolumeError, part_off, parts};

use crate::i18n::tr;
use crate::job::{Worker, pooled};

/// One model to split into its parts, already placed on the plate.
pub struct SplitRequest {
    pub placed: Mesh,
    pub settings: PartSettings,
    /// The point on the plate a part is marked at, to cut that part off instead.
    pub at: Option<Vec3>,
}

/// How a split ended.
#[derive(Debug)]
pub enum SplitOutcome {
    Split(Parts),
    PartedOff(PartedOff),
    Failed(String),
}

/// A split on its own thread: cutting off a marked part builds a field of the model and
/// cuts it exactly, seconds on a real one.
pub type SplitJob = Worker<SplitOutcome>;

/// Starts the split. The thread is detached: it ends on its own.
pub fn spawn_split(request: SplitRequest) -> SplitJob {
    Worker::spawn(
        move |stop, progress| pooled(|| run(request, stop, progress), SplitOutcome::Failed),
        || SplitOutcome::Failed(tr!("job-thread-stopped-split")),
    )
}

fn run(request: SplitRequest, stop: &AtomicBool, progress: &Progress) -> SplitOutcome {
    let asked = || stop.load(Ordering::Relaxed);
    let Some(at) = request.at else {
        return SplitOutcome::Split(parts(&request.placed, &request.settings));
    };
    match part_off(
        &request.placed,
        at,
        Cancel::when(&asked).reporting(progress),
    ) {
        Ok(parted) => SplitOutcome::PartedOff(parted),
        Err(VolumeError::NoNeck | VolumeError::NotOnModel) => {
            SplitOutcome::Failed(tr!("cut-no-neck"))
        }
        Err(error) => SplitOutcome::Failed(tr!("cut-split-failed", reason = error.to_string())),
    }
}
