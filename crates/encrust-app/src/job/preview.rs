use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use core_geometry::Mesh;
use core_slicer::Windows;

use crate::job::pipeline::{Cutting, thread_pool, windows_of, worker_threads};

/// How a preview build ended.
#[derive(Debug, Clone, PartialEq)]
pub enum PreviewOutcome {
    /// The plate as one mesh, and the layers it will be cut into. No contours: the layer
    /// being looked at is cut when it is looked at; see
    /// `docs/decisions/0068-the-window-holds-no-stack.md`.
    Built(Arc<Mesh>, Windows),
    /// The whole error chain flattened into one line, as the window has no terminal.
    Failed(String),
}

/// The plate merged and its layer heights worked out, on its own thread.
///
/// There is no cancellation flag: the window cancels by dropping the handle, which
/// leaves the thread to finish into a closed channel.
pub struct PreviewJob {
    result: Receiver<PreviewOutcome>,
}

impl PreviewJob {
    /// Starts cutting `mesh`, which already stands in plate coordinates.
    pub fn spawn(mesh: Mesh, cutting: Cutting) -> Self {
        let (sender, result) = mpsc::channel();

        std::thread::spawn(move || {
            let _ = sender.send(build(mesh, cutting));
        });

        Self { result }
    }

    /// The outcome on the frame the build ends, and `None` while it is still running.
    pub fn poll(&mut self) -> Option<PreviewOutcome> {
        match self.result.try_recv() {
            Ok(outcome) => Some(outcome),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(PreviewOutcome::Failed(
                "the preview thread stopped without finishing".to_owned(),
            )),
        }
    }
}

fn build(mesh: Mesh, cutting: Cutting) -> PreviewOutcome {
    let pool = match thread_pool(worker_threads()) {
        Ok(pool) => pool,
        Err(error) => return PreviewOutcome::Failed(error.to_string()),
    };

    match pool.install(|| windows_of(&mesh, cutting)) {
        Ok(windows) => PreviewOutcome::Built(Arc::new(mesh), windows),
        Err(error) => PreviewOutcome::Failed(
            error
                .chain()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(": "),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::Vec3;

    /// A regular tetrahedron of unit edges standing on the plate.
    fn tetrahedron() -> Mesh {
        Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
            ],
            vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
        )
    }

    /// Blocks until the job reports, which it always does: the thread sends an outcome
    /// on every path, and a thread that panicked drops the sender.
    fn wait(job: &mut PreviewJob) -> PreviewOutcome {
        loop {
            if let Some(outcome) = job.poll() {
                return outcome;
            }
            std::thread::yield_now();
        }
    }

    #[test]
    fn a_build_comes_back_as_the_layers_the_plate_will_be_cut_into() {
        let mut job = PreviewJob::spawn(tetrahedron(), Cutting::uniform(0.25));
        let PreviewOutcome::Built(mesh, windows) = wait(&mut job) else {
            panic!("a sound mesh slices");
        };
        // One millimetre of model at a quarter of a millimetre a layer.
        assert_eq!(windows.layer_count(), 4);
        assert!(windows.height_of(0) < windows.height_of(3));
        assert_eq!(mesh.faces.len(), 4, "the plate comes back to cut from");
    }

    #[test]
    fn a_layer_height_of_zero_comes_back_as_a_failure() {
        let mut job = PreviewJob::spawn(tetrahedron(), Cutting::uniform(0.0));
        let PreviewOutcome::Failed(message) = wait(&mut job) else {
            panic!("a layer height of zero cannot produce a stack");
        };
        assert!(message.contains("cannot slice the model"), "got {message}");
    }

    #[test]
    fn a_job_still_running_reports_nothing_yet() {
        let (_sender, result) = mpsc::channel();
        let mut job = PreviewJob { result };
        assert_eq!(job.poll(), None);
    }

    #[test]
    fn a_worker_that_dies_without_an_outcome_is_a_failure() {
        let (sender, result) = mpsc::channel();
        let mut job = PreviewJob { result };
        drop(sender);
        let Some(PreviewOutcome::Failed(message)) = job.poll() else {
            panic!("a dead worker must end the build");
        };
        assert!(message.contains("stopped"), "got {message}");
    }
}
