use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};

use crate::plate::BuildPlate;
use crate::scene::Imported;

/// What an import is doing. Reading is the file, repairing is welding and orienting, and
/// indexing is the hierarchy every later ray goes through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportStage {
    Reading,
    Repairing,
    Indexing,
}

impl ImportStage {
    pub fn label(self) -> &'static str {
        match self {
            Self::Reading => "Reading",
            Self::Repairing => "Repairing",
            Self::Indexing => "Indexing",
        }
    }
}

/// How one import ended.
#[derive(Debug)]
pub enum ImportOutcome {
    /// Boxed because it carries a whole model, and the channel holds one of these per
    /// message either way.
    Opened(Box<Imported>),
    /// The whole error chain flattened into one line, as the window has no terminal.
    Failed(String),
}

#[derive(Debug)]
enum Report {
    Stage(ImportStage),
    Finished(ImportOutcome),
}

/// One file being opened on its own thread.
///
/// Reading, welding, orienting and indexing a million-triangle model is seconds of work,
/// which is far too much for a frame; see `docs/decisions/0038`.
#[derive(Debug)]
pub struct ImportJob {
    reports: Receiver<Report>,
    path: PathBuf,
    stage: ImportStage,
}

impl ImportJob {
    /// Starts the import. The thread is detached: nothing can cancel a read that is
    /// already under way, and an import that finishes into a dropped handle is dropped
    /// with it.
    pub fn spawn(path: PathBuf, plate: BuildPlate) -> Self {
        let (sender, reports) = mpsc::channel();
        let worker_path = path.clone();

        std::thread::spawn(move || {
            let outcome = match crate::import::prepare(&worker_path, &plate, &mut |stage| {
                let _ = sender.send(Report::Stage(stage));
            }) {
                Ok(imported) => ImportOutcome::Opened(Box::new(imported)),
                Err(error) => ImportOutcome::Failed(
                    error
                        .chain()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(": "),
                ),
            };
            let _ = sender.send(Report::Finished(outcome));
        });

        Self {
            reports,
            path,
            stage: ImportStage::Reading,
        }
    }

    /// Takes everything the worker reported since the last frame, and returns the outcome
    /// on the frame the import ends.
    pub fn poll(&mut self) -> Option<ImportOutcome> {
        loop {
            match self.reports.try_recv() {
                Ok(Report::Stage(stage)) => self.stage = stage,
                Ok(Report::Finished(outcome)) => return Some(outcome),
                Err(TryRecvError::Empty) => return None,
                Err(TryRecvError::Disconnected) => {
                    return Some(ImportOutcome::Failed(
                        "the import thread stopped without finishing".to_owned(),
                    ));
                }
            }
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn label(&self) -> String {
        let name = self.path.file_name().map_or_else(
            || self.path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        format!("{} {name}", self.stage.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_that_is_not_there_comes_back_as_a_failure() {
        let mut job = ImportJob::spawn(PathBuf::from("no-such-model.stl"), BuildPlate::default());
        let outcome = loop {
            if let Some(outcome) = job.poll() {
                break outcome;
            }
            std::thread::yield_now();
        };

        let ImportOutcome::Failed(message) = outcome else {
            panic!("a missing file cannot open");
        };
        assert!(message.contains("no-such-model.stl"), "got {message}");
    }

    #[test]
    fn a_running_import_says_which_file_it_is_on() {
        let job = ImportJob::spawn(PathBuf::from("/models/dragon.stl"), BuildPlate::default());
        assert!(job.label().ends_with("dragon.stl"), "got {}", job.label());
        assert_eq!(job.path(), Path::new("/models/dragon.stl"));
    }
}
