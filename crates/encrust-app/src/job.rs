use crate::i18n::tr;
mod boolean;
mod cut;
mod format;
mod hollow;
mod import;
mod measure;
mod orient;
mod pipeline;
mod place;
mod plate;
mod preview;
mod relief;
mod repair;
mod send;
mod split;
mod trapped;

pub use boolean::{BooleanJob, BooleanOutcome, BooleanRequest, Method};
pub use core_pipeline::SlicedFormat;
pub use cut::{CutJob, CutOutcome, CutRequest, spawn_cut};
#[cfg(not(target_arch = "wasm32"))]
pub use format::applied_to;
pub use format::label_of;
pub use hollow::{
    HollowJob, HollowOutcome, HollowRequest, SharedFields, hollow_tasks, lock as lock_fields,
    rebuild_tasks,
};
pub use import::{ImportJob, ImportOutcome, ImportStage};
pub use measure::{MeasureJob, MeasureOutcome};
pub use orient::{
    Costs, OrientJob, OrientOutcome, OrientRequest, OrientTask, Pose, Proof, Turn, orient_tasks,
};
#[cfg(target_arch = "wasm32")]
pub use pipeline::run_into;
pub use pipeline::{SliceRequest, worker_threads};
pub use place::{SupportJob, SupportOutcome, SupportRequest, tasks_of};
pub use plate::models_of;
pub use preview::{PreviewJob, PreviewOutcome};
pub use relief::{
    Pressed, ReliefJob, ReliefOutcome, ReliefRequest, ReliefSource, relief_tasks, spawn_relief,
};
pub use repair::{RepairJob, RepairOutcome, RepairRequest, Repaired, mend};
pub use send::{Action, SendJob, SendOutcome, SendRequest};
pub use split::{SplitJob, SplitOutcome, SplitRequest, spawn_split};
pub use trapped::{TrapJob, TrapOutcome, TrapRequest, trap_tasks};

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};

/// Runs `work` off the thread that draws: on a thread of its own at the desk, and in a
/// browser on a worker, since the page's thread may never wait; see ADR 0181.
pub fn spawn(work: impl FnOnce() + Send + 'static) {
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::spawn(work);
    #[cfg(target_arch = "wasm32")]
    crate::web::thread::spawn(work);
}

/// A run on a thread of its own that ends with one value, polled for it once a frame.
///
/// Dropping it asks the run to stop: `work` is handed the flag, for the steps that can be
/// given up on part way, and whatever it ends with is thrown away.
#[derive(Debug)]
pub struct Worker<T> {
    outcome: Receiver<T>,
    /// What the run is taken to have ended with when its thread stops without a word.
    lost: fn() -> T,
    stop: Arc<AtomicBool>,
}

impl<T: Send + 'static> Worker<T> {
    /// Starts `work`. The thread is detached: it ends on its own.
    pub fn spawn(work: impl FnOnce(&AtomicBool) -> T + Send + 'static, lost: fn() -> T) -> Self {
        let (sender, outcome) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let asked = Arc::clone(&stop);
        spawn(move || {
            let _ = sender.send(work(&asked));
        });
        Self {
            outcome,
            lost,
            stop,
        }
    }

    /// Returns the outcome on the frame the run ends, and `None` while it is going.
    pub fn poll(&mut self) -> Option<T> {
        match self.outcome.try_recv() {
            Ok(outcome) => Some(outcome),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some((self.lost)()),
        }
    }
}

impl<T> Drop for Worker<T> {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Runs `work` with its parallel loops on a pool of the worker threads, or answers
/// `failed` with why the pool could not start.
pub fn pooled<T: Send>(work: impl FnOnce() -> T + Send, failed: impl FnOnce(String) -> T) -> T {
    match pipeline::thread_pool(worker_threads()) {
        Ok(pool) => pool.install(work),
        Err(error) => failed(format!("{error:#}")),
    }
}

/// The cores the machine has.
#[cfg(not(target_arch = "wasm32"))]
pub fn cores() -> usize {
    std::thread::available_parallelism().map_or(1, std::num::NonZero::get)
}

/// The cores the machine has, which a browser tells the page rather than the standard
/// library.
#[cfg(target_arch = "wasm32")]
pub fn cores() -> usize {
    crate::web::thread::cores()
}

/// What the worker is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Slicing,
    Rasterising,
}

impl Stage {
    pub fn label(self) -> String {
        match self {
            Self::Slicing => tr!("slice-slicing"),
            Self::Rasterising => tr!("job-rasterising"),
        }
    }
}

/// One message from the worker to the window.
#[derive(Debug, Clone, PartialEq)]
pub enum Progress {
    Stage(Stage),
    /// `done` layers of `total` have reached the file.
    Layers {
        done: usize,
        total: usize,
    },
    Finished(Outcome),
}

/// How a slicing job ended.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Written {
        path: PathBuf,
        layers: usize,
        /// Layers that reached past the edge of the panel and were cut off.
        clipped_layers: usize,
        volume_mm3: f32,
    },
    Cancelled,
    /// The whole error chain flattened into one line: the window has no terminal to
    /// print the causes to.
    Failed(String),
}

/// A slicing job running on its own thread, and what it has reported so far.
///
/// The window owns the handle and drains it once a frame; see
/// `docs/decisions/0018-background-slicing-job.md`.
pub struct SliceJob {
    progress: Receiver<Progress>,
    cancel: Arc<AtomicBool>,
    stage: Stage,
    done: usize,
    total: usize,
}

impl SliceJob {
    /// Starts the job. The thread is detached: it ends on its own, and dropping the
    /// handle cancels it.
    pub fn spawn(request: SliceRequest) -> Self {
        let (sender, progress) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);

        #[cfg(not(target_arch = "wasm32"))]
        spawn(move || {
            let outcome = pipeline::run(&request, &worker_cancel, &mut |message| {
                let _ = sender.send(message);
            });
            let _ = sender.send(Progress::Finished(outcome));
        });
        #[cfg(target_arch = "wasm32")]
        crate::web::thread::spawn_async(move || async move {
            let outcome = crate::web::files::slice_offered(&request, &worker_cancel, &mut |m| {
                let _ = sender.send(m);
            })
            .await;
            let _ = sender.send(Progress::Finished(outcome));
        });

        Self {
            progress,
            cancel,
            stage: Stage::Slicing,
            done: 0,
            total: 0,
        }
    }

    /// Takes everything the worker reported since the last frame, and returns the outcome
    /// on the frame the job ends.
    ///
    /// A worker that panics drops its end of the channel without sending an outcome. That
    /// is reported as a failure rather than waited on, or the progress bar would sit
    /// there for the rest of the session.
    pub fn poll(&mut self) -> Option<Outcome> {
        loop {
            match self.progress.try_recv() {
                Ok(Progress::Stage(stage)) => self.stage = stage,
                Ok(Progress::Layers { done, total }) => {
                    self.done = done;
                    self.total = total;
                }
                Ok(Progress::Finished(outcome)) => return Some(outcome),
                Err(TryRecvError::Empty) => return None,
                Err(TryRecvError::Disconnected) => {
                    return Some(Outcome::Failed(tr!("job-thread-stopped")));
                }
            }
        }
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// The job has been told to stop but has not reached its next cancellation point yet.
    pub fn is_cancelling(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// Share of the layers written, or `None` while slicing, when the layer count is not
    /// known yet.
    pub fn fraction(&self) -> Option<f32> {
        (self.total > 0).then(|| self.done as f32 / self.total as f32)
    }

    pub fn label(&self) -> String {
        if self.is_cancelling() {
            return tr!("job-cancelling");
        }
        match self.fraction() {
            Some(_) => tr!(
                "job-progress",
                stage = self.stage.label(),
                done = self.done,
                total = self.total
            ),
            None => tr!("job-working", stage = self.stage.label()),
        }
    }
}

/// A job whose handle is gone has nobody left to report to, so it is stopped rather than
/// left rasterising a file nothing will look at.
impl Drop for SliceJob {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A handle with no worker behind it, so that the counters can be driven by hand.
    fn idle_job(progress: Receiver<Progress>) -> SliceJob {
        SliceJob {
            progress,
            cancel: Arc::new(AtomicBool::new(false)),
            stage: Stage::Slicing,
            done: 0,
            total: 0,
        }
    }

    fn job() -> SliceJob {
        let (_sender, progress) = mpsc::channel();
        idle_job(progress)
    }

    #[test]
    fn a_job_without_a_layer_count_has_no_fraction() {
        let job = job();
        assert_eq!(job.fraction(), None);
        assert_eq!(job.label(), "Slicing...");
    }

    #[test]
    fn the_fraction_follows_the_layers_written() {
        let mut job = job();
        job.stage = Stage::Rasterising;
        job.done = 3;
        job.total = 4;
        assert_eq!(job.fraction(), Some(0.75));
        assert_eq!(job.label(), "Rasterising layer 3 of 4");
    }

    #[test]
    fn cancelling_shows_in_the_label() {
        let job = job();
        job.cancel();
        assert!(job.is_cancelling());
        assert_eq!(job.label(), "Cancelling");
    }

    #[test]
    fn polling_keeps_the_last_counts_and_reports_the_outcome_once() {
        let (sender, progress) = mpsc::channel();
        let mut job = idle_job(progress);
        sender
            .send(Progress::Stage(Stage::Rasterising))
            .expect("the job holds the receiver");
        sender
            .send(Progress::Layers { done: 2, total: 8 })
            .expect("the job holds the receiver");
        sender
            .send(Progress::Finished(Outcome::Cancelled))
            .expect("the job holds the receiver");

        assert_eq!(job.poll(), Some(Outcome::Cancelled));
        assert_eq!(job.fraction(), Some(0.25));
        drop(sender);
    }

    #[test]
    fn a_worker_that_dies_without_an_outcome_is_a_failure() {
        let (sender, progress) = mpsc::channel();
        let mut job = idle_job(progress);
        assert_eq!(job.poll(), None, "an empty channel means still running");

        drop(sender);
        let Some(Outcome::Failed(message)) = job.poll() else {
            panic!("a dead worker must end the job");
        };
        assert!(message.contains("stopped"), "got {message}");
    }
}
