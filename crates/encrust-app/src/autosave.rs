//! A copy of the plate kept while it is not saved, offered back at the next start when a run
//! ended without closing; see `docs/decisions/0452`.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime};

use core_engine::project::{EXTENSION, Project, ProjectError};
use web_time::Instant;

use crate::i18n::tr;
use crate::panels::Window;
use crate::status::Status;

/// How long the plate goes between looks for a change to copy.
pub const EVERY: Duration = Duration::from_secs(120);

/// Copies the plate aside while it is not saved, so a run that ends without closing loses
/// at most [`EVERY`] of work; says once when a copy cannot be written.
pub fn tick(window: &mut Window, ctx: &egui::Context) {
    ctx.request_repaint_after(EVERY);
    if let Some(error) = window.machine.autosave.poll() {
        window.machine.status =
            Status::Error(tr!("autosave-cannot-write", reason = error.to_string()));
    }
    if !window.machine.autosave.due(Instant::now()) {
        return;
    }
    let unsaved = window
        .machine
        .general
        .autosave
        .then(|| crate::project::unsaved(window))
        .flatten();
    let belonged = window.doc.project.path.clone();
    window.machine.autosave.keep(unsaved, belonged);
}

/// This run's copy of the plate, and the copies runs that ended without closing left.
#[derive(Default)]
pub struct Autosave {
    /// Where this run keeps its copy, or `None` where nothing can be kept: a browser, a
    /// system without a configuration folder, a folder that cannot be written.
    slot: Option<Slot>,
    looked: Option<Instant>,
    /// The digest of the plate last copied, so an unchanged plate is not written again.
    written: Option<u64>,
    writing: Option<JoinHandle<Result<(), ProjectError>>>,
    /// The last write failed, so the next failure is not said again.
    failed: bool,
    /// Copies other runs left, newest first, each held by its lock while this run has it.
    found: Vec<Left>,
}

/// One run's files: the copy, the project it belonged to, and the lock held while it runs.
struct Slot {
    dir: PathBuf,
    stem: String,
    _lock: File,
}

/// A copy a run left without closing.
pub struct Left {
    dir: PathBuf,
    stem: String,
    /// The project file the plate belonged to, or `None` for one never saved.
    pub belonged: Option<PathBuf>,
    /// When it was written, seconds since the Unix epoch.
    pub at_s: u64,
    lock: File,
}

impl Autosave {
    /// Takes this run's slot beside the user's profiles, and finds what other runs left.
    pub fn start() -> Self {
        dir().map_or_else(Self::default, |dir| Self::in_dir(&dir))
    }

    fn in_dir(dir: &Path) -> Self {
        if std::fs::create_dir_all(dir).is_err() {
            return Self::default();
        }
        Self {
            found: left_in(dir),
            slot: Slot::take(dir).ok(),
            ..Self::default()
        }
    }

    /// Whether the plate is due a look: once every [`EVERY`], counted from the first frame,
    /// and never while the last copy is still being written.
    pub fn due(&mut self, now: Instant) -> bool {
        if self.slot.is_none() || self.writing.is_some() {
            return false;
        }
        let looked = *self.looked.get_or_insert(now);
        if now.duration_since(looked) < EVERY {
            return false;
        }
        self.looked = Some(now);
        true
    }

    /// Copies `unsaved`, with the project file it `belonged` to, on a thread of its own when
    /// it changed since the last copy; drops the copy when there is nothing unsaved.
    pub fn keep(&mut self, unsaved: Option<(Project, u64)>, belonged: Option<PathBuf>) {
        let Some(slot) = &self.slot else {
            return;
        };
        let Some((project, digest)) = unsaved else {
            slot.remove_copy();
            self.written = None;
            return;
        };
        if self.written == Some(digest) {
            return;
        }
        self.written = Some(digest);
        let files = slot.files();
        self.writing = Some(std::thread::spawn(move || {
            files.write(&project, belonged.as_deref())
        }));
    }

    /// The error of a copy that has just failed to write, said once until one succeeds.
    pub fn poll(&mut self) -> Option<ProjectError> {
        if !self.writing.as_ref().is_some_and(JoinHandle::is_finished) {
            return None;
        }
        let Ok(written) = self.writing.take()?.join() else {
            self.written = None;
            return None;
        };
        let was_failing = std::mem::replace(&mut self.failed, written.is_err());
        if written.is_err() {
            // Written again at the next look rather than taken for done.
            self.written = None;
        }
        written.err().filter(|_| !was_failing)
    }

    /// The window closed as it was asked to: its copy goes, since nothing was lost.
    pub fn finish(&mut self) {
        self.wait();
        if let Some(slot) = self.slot.take() {
            slot.remove_all();
        }
    }

    /// The copy offered back, the newest another run left.
    pub fn offered(&self) -> Option<&Left> {
        self.found.first()
    }

    /// The offered copy, taken off the list to be restored or discarded.
    pub fn take_offered(&mut self) -> Option<Left> {
        (!self.found.is_empty()).then(|| self.found.remove(0))
    }

    /// Takes a restored copy as this run's own, so it survives this run closing badly too.
    pub fn adopt(&mut self, left: Left) {
        self.wait();
        match &self.slot {
            Some(slot) => {
                let files = slot.files();
                let _ = std::fs::rename(left.copy(), files.copy);
                let _ = std::fs::rename(file(&left.dir, &left.stem, BELONGED), files.belonged);
                left.discard();
            }
            None => drop(left),
        }
        self.written = None;
    }

    /// Lets the copy being written land, so nothing is renamed over it afterwards.
    fn wait(&mut self) {
        if let Some(writing) = self.writing.take() {
            let _ = writing.join();
        }
    }
}

impl Left {
    /// Its copy of the plate, a project file.
    pub fn copy(&self) -> PathBuf {
        file(&self.dir, &self.stem, EXTENSION)
    }

    /// Deletes what the run left.
    pub fn discard(self) {
        let Self {
            dir, stem, lock, ..
        } = self;
        drop(lock);
        remove_run(&dir, &stem);
    }

    /// The copy `stem` in `dir`, when the run that wrote it holds its lock no longer.
    fn claim(dir: &Path, stem: &str) -> Option<Self> {
        let lock = File::options()
            .read(true)
            .write(true)
            .open(file(dir, stem, LOCK))
            .ok()?;
        // A lock still held is a run still going.
        lock.try_lock().ok()?;
        let at_s = std::fs::metadata(file(dir, stem, EXTENSION))
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|at| at.duration_since(SystemTime::UNIX_EPOCH).ok());
        let Some(at_s) = at_s else {
            drop(lock);
            remove_run(dir, stem);
            return None;
        };
        let belonged = std::fs::read_to_string(file(dir, stem, BELONGED))
            .ok()
            .filter(|path| !path.is_empty())
            .map(PathBuf::from);
        Some(Self {
            dir: dir.to_path_buf(),
            stem: stem.to_owned(),
            belonged,
            at_s: at_s.as_secs(),
            lock,
        })
    }
}

impl Slot {
    /// A slot of this run's own, named so that no other run, past or present, has it.
    fn take(dir: &Path) -> std::io::Result<Self> {
        let started_ms = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let stem = format!("{}-{started_ms}", std::process::id());
        let taking = file(dir, &stem, TAKING);
        let lock = File::create(&taking)?;
        lock.try_lock()?;
        // Named a lock only once held, or a run starting beside it would take it for one gone.
        std::fs::rename(taking, file(dir, &stem, LOCK))?;
        Ok(Self {
            dir: dir.to_path_buf(),
            stem,
            _lock: lock,
        })
    }

    fn files(&self) -> Files {
        Files {
            copy: file(&self.dir, &self.stem, EXTENSION),
            part: file(&self.dir, &self.stem, PART),
            belonged: file(&self.dir, &self.stem, BELONGED),
        }
    }

    fn remove_copy(&self) {
        let files = self.files();
        for path in [files.copy, files.part, files.belonged] {
            let _ = std::fs::remove_file(path);
        }
    }

    fn remove_all(self) {
        let Self { dir, stem, _lock } = self;
        drop(_lock);
        remove_run(&dir, &stem);
    }
}

/// Where a copy is written, whole, before it takes the copy's name.
struct Files {
    copy: PathBuf,
    part: PathBuf,
    belonged: PathBuf,
}

impl Files {
    /// Writes the copy aside, on disk before it is renamed over the last one, so a run cut
    /// off while writing leaves the copy before it whole.
    fn write(&self, project: &Project, belonged: Option<&Path>) -> Result<(), ProjectError> {
        core_engine::project::save(&self.part, project)?;
        // Windows flushes only a handle open for writing.
        File::options().write(true).open(&self.part)?.sync_all()?;
        // A path that is not Unicode would not read back as itself: its plate comes back unnamed.
        let belonged = belonged.and_then(Path::to_str).unwrap_or_default();
        std::fs::write(&self.belonged, belonged)?;
        std::fs::rename(&self.part, &self.copy)?;
        Ok(())
    }
}

const LOCK: &str = "lock";
const TAKING: &str = "taking";
const PART: &str = "part";
const BELONGED: &str = "belonged";

fn file(dir: &Path, stem: &str, extension: &str) -> PathBuf {
    dir.join(format!("{stem}.{extension}"))
}

fn remove_run(dir: &Path, stem: &str) {
    for extension in [EXTENSION, PART, BELONGED, LOCK] {
        let _ = std::fs::remove_file(file(dir, stem, extension));
    }
}

/// The copies in `dir` whose runs are gone, newest first.
fn left_in(dir: &Path) -> Vec<Left> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<Left> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            (path.extension()? == LOCK).then_some(())?;
            Left::claim(dir, path.file_stem()?.to_str()?)
        })
        .collect();
    found.sort_by_key(|left| std::cmp::Reverse(left.at_s));
    found
}

/// A browser has no folder to keep it in.
#[cfg(not(target_arch = "wasm32"))]
fn dir() -> Option<PathBuf> {
    Some(printer_profiles::user_dir()?.with_file_name("recovery"))
}

#[cfg(target_arch = "wasm32")]
fn dir() -> Option<PathBuf> {
    None
}

#[cfg(test)]
mod tests;
