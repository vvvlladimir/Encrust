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
mod send;
mod trapped;

pub use core_pipeline::SlicedFormat;
#[cfg(not(target_arch = "wasm32"))]
pub use format::applied_to;
pub use format::label_of;
pub use hollow::{HollowJob, HollowOutcome, HollowRequest, HollowTask, hollow_tasks};
pub use import::{ImportJob, ImportOutcome, ImportStage};
pub use measure::{MeasureJob, MeasureOutcome};
pub use orient::{OrientJob, OrientOutcome, OrientRequest, orient_tasks};
#[cfg(target_arch = "wasm32")]
pub use pipeline::run_into;
pub use pipeline::{SliceRequest, worker_threads};
pub use place::{SupportJob, SupportOutcome, SupportRequest, tasks_of};
pub use plate::models_of;
pub use preview::{PreviewJob, PreviewOutcome};
pub use relief::{ReliefJob, ReliefOutcome, ReliefRequest, relief_tasks};
pub use send::{Action, SendJob, SendOutcome, SendRequest, Wire};
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
    pub fn label(self) -> &'static str {
        match self {
            Self::Slicing => "Slicing",
            Self::Rasterising => "Rasterising",
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
                    return Some(Outcome::Failed(
                        "the slicing thread stopped without finishing".to_owned(),
                    ));
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
            return "Cancelling".to_owned();
        }
        match self.fraction() {
            Some(_) => format!(
                "{} layer {} of {}",
                self.stage.label(),
                self.done,
                self.total
            ),
            None => format!("{}...", self.stage.label()),
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
