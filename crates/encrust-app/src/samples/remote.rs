//! The test prints too large to bundle: a picture in the window, the model fetched from the
//! repository's `test-prints/` on the first click and opened from disk on the next; see
//! `docs/decisions/0394`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use crate::i18n::tr;
use crate::status::Status;
use crate::updates::{Progress, UpdateError, fetch_counted};

/// One test print kept in the repository rather than in the window.
pub struct Remote {
    /// Its file name in `test-prints/`, and on disk once fetched.
    file: &'static str,
    name: fn() -> String,
    about: fn() -> String,
    /// Its picture as shade and coverage, the PNG a recent file's is kept as.
    pub picture_png: &'static [u8],
}

/// The test prints fetched from the repository, shown before [`super::SAMPLES`].
#[cfg(not(target_arch = "wasm32"))]
pub const REMOTES: [Remote; 1] = [Remote {
    file: "crustling.stl",
    name: || tr!("sample-crustling"),
    about: || tr!("sample-crustling-about"),
    picture_png: include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/samples/crustling.png"
    )),
}];

/// A browser has nowhere to keep one, and handing it over would download it; see ADR 0394.
#[cfg(target_arch = "wasm32")]
pub const REMOTES: [Remote; 0] = [];

impl Remote {
    /// What the test print is called, in the window's language.
    pub fn name(&self) -> String {
        (self.name)()
    }

    /// What printing it tells, shown when the pointer rests on its name.
    pub fn about(&self) -> String {
        (self.about)()
    }

    /// Where the repository serves it.
    pub fn url(&self) -> String {
        format!(
            "{}/raw/main/test-prints/{}",
            env!("CARGO_PKG_REPOSITORY"),
            self.file
        )
    }

    /// Where it is kept once fetched, or `None` where nothing can be kept: a browser.
    pub fn path(&self) -> Option<PathBuf> {
        dir().map(|dir| dir.join(self.file))
    }

    /// Where it is kept, once it has been fetched.
    pub fn fetched(&self) -> Option<PathBuf> {
        self.path().filter(|path| path.is_file())
    }
}

/// Why a test print could not be fetched.
#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    /// The download failed or was cancelled.
    #[error(transparent)]
    Fetch(#[from] UpdateError),
    /// The fetched file could not be put on disk.
    #[error("{}", tr!("start-download-write", path = .path.display().to_string()))]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// One fetch under way, polled once a frame.
#[derive(Debug)]
pub struct Fetching {
    pub progress: Arc<Progress>,
    reply: Receiver<Result<(), FetchError>>,
}

impl Fetching {
    /// Fetches `url` into `path` on a thread of its own.
    pub fn start(url: String, path: PathBuf) -> Self {
        let progress = Arc::new(Progress::default());
        let counted = Arc::clone(&progress);
        let (sender, reply) = mpsc::channel();
        crate::job::spawn(move || {
            let fetched = fetch_counted(&url, &counted)
                .map_err(FetchError::from)
                .and_then(|bytes| keep(&path, &bytes));
            let _ = sender.send(fetched);
        });
        Self { progress, reply }
    }

    /// How much has come, of a size the server stated.
    pub fn share(&self) -> Option<f32> {
        let (received, total) = self.progress.bytes();
        total.map(|total| received as f32 / total as f32)
    }

    /// How it ended, once it has; a thread gone without a word ended in a cancel.
    pub fn ended(&self) -> Option<Result<(), FetchError>> {
        match self.reply.try_recv() {
            Ok(ended) => Some(ended),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err(UpdateError::Cancelled.into())),
        }
    }
}

/// The fetches running, one place for each of [`REMOTES`].
#[derive(Debug, Default)]
pub struct Fetches {
    running: [Option<Fetching>; REMOTES.len()],
}

impl Fetches {
    /// The fetch of the `index`th remote test print, while it runs.
    pub fn get(&self, index: usize) -> Option<&Fetching> {
        self.running.get(index)?.as_ref()
    }

    /// Starts fetching the `index`th remote test print, unless it is already coming or
    /// there is nowhere to keep it.
    pub fn start(&mut self, index: usize) {
        let remote = &REMOTES[index];
        if let (Some(slot @ None), Some(path)) = (self.running.get_mut(index), remote.path()) {
            *slot = Some(Fetching::start(remote.url(), path));
        }
    }

    /// Clears the fetches that have ended, saying on `status` why one failed, and answers
    /// whether one is still running.
    pub fn poll(&mut self, status: &mut Status) -> bool {
        for (index, slot) in self.running.iter_mut().enumerate() {
            if let Some(ended) = slot.as_ref().and_then(Fetching::ended) {
                *slot = None;
                if let Err(error) = ended {
                    let name = REMOTES[index].name();
                    let error = error.to_string();
                    *status =
                        Status::Error(tr!("start-download-failed", name = name, error = error));
                }
            }
        }
        self.running.iter().any(Option::is_some)
    }
}

/// Written beside its place and renamed into it, so a half-fetched file is never opened.
fn keep(path: &Path, bytes: &[u8]) -> Result<(), FetchError> {
    let written = || -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let partial = path.with_extension("part");
        std::fs::write(&partial, bytes)?;
        std::fs::rename(partial, path)
    };
    written().map_err(|source| FetchError::Write {
        path: path.to_owned(),
        source,
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn dir() -> Option<PathBuf> {
    Some(printer_profiles::user_dir()?.with_file_name("test-prints"))
}

#[cfg(target_arch = "wasm32")]
fn dir() -> Option<PathBuf> {
    None
}

#[cfg(test)]
mod tests;
