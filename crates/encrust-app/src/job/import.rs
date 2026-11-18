use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed};
use std::sync::mpsc::{self, Receiver, TryRecvError};

use crate::files::Handed;
use crate::i18n::tr;
use crate::plate::BuildPlate;
use crate::scene::Imported;

/// What an import is doing: reading the file, repairing a mesh, surfacing a sliced file's
/// layers, or indexing the hierarchy every later ray goes through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportStage {
    Reading,
    Repairing,
    Surfacing,
    Indexing,
}

impl ImportStage {
    pub fn label(self) -> String {
        match self {
            Self::Reading => tr!("import-stage-reading"),
            Self::Repairing => tr!("import-stage-repairing"),
            Self::Surfacing => tr!("import-stage-surfacing"),
            Self::Indexing => tr!("import-stage-indexing"),
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

/// What an import's thread is asked and tells besides its stage: whether to stop, and how
/// far a sliced file's rebuild has got. A mesh's import counts nothing.
#[derive(Debug, Default)]
pub struct ImportWatch {
    stop: AtomicBool,
    layers_done: AtomicUsize,
    layers: AtomicUsize,
    /// The surface read out of the rebuilt field.
    pub surface: core_volume::Progress,
}

impl ImportWatch {
    /// Asks the import to stop at its next layer or layer of tiles.
    pub fn stop(&self) {
        self.stop.store(true, Relaxed);
    }

    /// Whether the import has been asked to stop.
    pub fn stopped(&self) -> bool {
        self.stop.load(Relaxed)
    }

    /// `done` of a file's `layers` are built back.
    pub fn layers(&self, done: usize, layers: usize) {
        self.layers.store(layers, Relaxed);
        self.layers_done.store(done, Relaxed);
    }

    /// The share done, or `None` while the import has counted nothing.
    pub fn share(&self) -> Option<f32> {
        // Weighed on a 16K plate of 521 layers: the layers two fifths, the surface the rest.
        const LAYERS: f32 = 0.4;
        if let Some((_, done, total)) = self.surface.read()
            && total > 0
        {
            return Some(LAYERS + (1.0 - LAYERS) * done as f32 / total as f32);
        }
        let layers = self.layers.load(Relaxed);
        (layers > 0).then(|| LAYERS * self.layers_done.load(Relaxed) as f32 / layers as f32)
    }
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
    file: Handed,
    stage: ImportStage,
    watch: Arc<ImportWatch>,
}

impl Drop for ImportJob {
    fn drop(&mut self) {
        self.watch.stop();
    }
}

impl ImportJob {
    /// Starts the import. The thread is detached: a sliced file's rebuild stops once the
    /// handle is dropped, a mesh's read runs on, and either outcome is dropped with it.
    pub fn spawn(file: Handed, plate: BuildPlate) -> Self {
        Self::run(file, move |file, stage, _| {
            crate::import::prepare(file, &plate, stage)
        })
    }

    /// Starts building a sliced file's layers back into a model, its panel lying as
    /// `placement` says.
    pub fn spawn_sliced(file: Handed, placement: crate::sliced::Placement) -> Self {
        Self::run(file, move |file, stage, watch| {
            crate::sliced::prepare(file, placement, stage, watch)
        })
    }

    fn run(
        file: Handed,
        work: impl FnOnce(
            &Handed,
            &mut dyn FnMut(ImportStage),
            &ImportWatch,
        ) -> anyhow::Result<Imported>
        + Send
        + 'static,
    ) -> Self {
        let (sender, reports) = mpsc::channel();
        let worker_file = file.clone();
        let watch = Arc::new(ImportWatch::default());
        let told = Arc::clone(&watch);

        crate::job::spawn(move || {
            let mut stage = |stage| {
                let _ = sender.send(Report::Stage(stage));
            };
            let outcome = match work(&worker_file, &mut stage, &told) {
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
            file,
            stage: ImportStage::Reading,
            watch,
        }
    }

    /// The share done, or `None` for an import that cannot count its work.
    pub fn fraction(&self) -> Option<f32> {
        self.watch.share()
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
                    return Some(ImportOutcome::Failed(tr!("job-thread-stopped-import")));
                }
            }
        }
    }

    pub fn path(&self) -> &Path {
        self.file.path()
    }

    /// Whether the file has a path it can be opened from again.
    pub fn is_on_disk(&self) -> bool {
        matches!(self.file, Handed::Path(_))
    }

    pub fn label(&self) -> String {
        let path = self.path();
        let name = path.file_name().map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        tr!("import-stage", stage = self.stage.label(), name = name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_that_is_not_there_comes_back_as_a_failure() {
        let mut job = ImportJob::spawn(
            Handed::Path("no-such-model.stl".into()),
            BuildPlate::default(),
        );
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
        let job = ImportJob::spawn(
            Handed::Path("/models/dragon.stl".into()),
            BuildPlate::default(),
        );
        assert!(job.label().ends_with("dragon.stl"), "got {}", job.label());
        assert_eq!(job.path(), Path::new("/models/dragon.stl"));
    }
}
