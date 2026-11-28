//! What a long-running `encrust serve` keeps between runs: files read, plates after each edit
//! and the projects it wrote, each under the hash of what made it (ADR 0467).

use std::collections::HashMap;
use std::fmt;
use std::fs::Metadata;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::UNIX_EPOCH;

use core_document::{Applied, Document};
use core_edit::{Edit, ObjectId};
use core_engine::Model;
use core_engine::project::{ModelMeshes, ObjectState, SupportState};
use core_mesh_io::Loaded;
use sha2::{Digest, Sha256};

use crate::estimate::Estimate;

mod kept;

use kept::Kept;

/// How many of each kind are kept, the least recently used dropped first; a plate after an
/// edit shares its unchanged meshes with the one before, so most of one is counted once.
const FILES: usize = 8;
const PLATES: usize = 48;
const MODELS: usize = 24;
const WRITTEN: usize = 16;
const CHECKS: usize = 8;
/// How much of each end of a file names it beside its length and time, bytes.
const SAMPLED: u64 = 64 * 1024;

/// The hash a step's result is kept under: of what it was made from, chained edit by edit.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Key([u8; 32]);

impl Key {
    /// The key of `parts` taken in order, each told apart from the next by its length.
    pub(crate) fn of(parts: &[&[u8]]) -> Self {
        let mut hasher = Sha256::new();
        for part in parts {
            hasher.update((part.len() as u64).to_le_bytes());
            hasher.update(part);
        }
        Self(hasher.finalize().into())
    }

    /// The key of what this one's result became after `step`.
    pub(crate) fn then(self, step: &[u8]) -> Self {
        Self::of(&[&self.0, step])
    }

    /// The key of this one's result taken with `other`'s.
    pub(crate) fn with(self, other: Self) -> Self {
        Self::of(&[&self.0, &other.0])
    }
}

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.0[..6] {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// A plate as an edit left it, with the entry each object came from and what the edit said.
#[derive(Clone)]
pub(crate) struct Edited {
    pub document: Document,
    pub owners: Vec<(ObjectId, usize)>,
    pub applied: Applied,
}

/// Everything a server keeps between runs; a run of its own keeps none.
#[derive(Default)]
pub struct Steps {
    kept: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    /// A file's content hash by its path and what names it on disk, so it is read in full once.
    named: HashMap<(PathBuf, Named), Key>,
    loaded: Kept<Key, Arc<Loaded>>,
    plates: Kept<Key, Edited>,
    /// Each with the meshes its key names by address, held so no other takes the address.
    models: Kept<Key, (Model, ModelMeshes)>,
    /// The projects written, by what names them on disk, with the plate each holds.
    written: Kept<Named, (Key, Document)>,
    /// What a plate's check found, by the plate's models and the machine they are cut for, with
    /// the meshes its key names by address.
    checks: Kept<Key, (Arc<Estimate>, Vec<ModelMeshes>)>,
}

/// A file as the disk describes it: its length and time, and the bytes at each end, which a
/// rename keeps and a rewrite does not.
#[derive(Clone, PartialEq, Eq, Hash)]
struct Named {
    len: u64,
    modified_ns: u128,
    ends: Key,
}

impl fmt::Debug for Steps {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Steps")
    }
}

impl Steps {
    fn inner(&self) -> MutexGuard<'_, Inner> {
        // A run that panicked holding the lock left whole entries: each is put in one call.
        self.kept
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The hash of the file at `path`, read in full only the first time it is seen as it is.
    pub(crate) fn file_key(&self, path: &Path) -> std::io::Result<Key> {
        let named = (path.to_path_buf(), named(path)?);
        if let Some(key) = self.inner().named.get(&named) {
            return Ok(*key);
        }
        let bytes = std::fs::read(path)?;
        let key = Key::of(&[b"file", &bytes]);
        let mut inner = self.inner();
        if inner.named.len() > 4 * PLATES {
            inner.named.clear();
        }
        inner.named.insert(named, key);
        Ok(key)
    }

    /// The file at `path` loaded by `load`, or as it was loaded before unchanged.
    pub(crate) fn loaded<E>(
        &self,
        path: &Path,
        load: impl FnOnce() -> Result<Loaded, E>,
    ) -> Result<Arc<Loaded>, E> {
        let Ok(key) = self.file_key(path) else {
            return load().map(Arc::new);
        };
        if let Some(loaded) = self.inner().loaded.get(&key) {
            return Ok(loaded);
        }
        let loaded = Arc::new(load()?);
        self.inner().loaded.put(key, Arc::clone(&loaded), FILES);
        Ok(loaded)
    }

    /// The plate `key` names, as an edit left it before.
    pub(crate) fn plate(&self, key: Key) -> Option<Edited> {
        self.inner().plates.get(&key)
    }

    pub(crate) fn keep_plate(&self, key: Key, edited: Edited) {
        self.inner().plates.put(key, edited, PLATES);
    }

    /// The object `object` and `meshes` describe as a run cuts it, its supports to `plate`'s
    /// groups, or as it was cut before unchanged.
    pub(crate) fn model(
        &self,
        (object, meshes, plate): (&ObjectState, &ModelMeshes, &SupportState),
        make: impl FnOnce() -> Option<Model>,
    ) -> Option<Model> {
        let Some(key) = object_key(object, meshes, plate) else {
            return make();
        };
        if let Some((model, _)) = self.inner().models.get(&key) {
            return Some(model);
        }
        let model = make()?;
        let kept = (model.clone(), meshes.clone());
        self.inner().models.put(key, kept, MODELS);
        Some(model)
    }

    /// Notes that the project at `path` was just written from `document`, the plate `key` names.
    pub(crate) fn wrote(&self, path: &Path, key: Key, document: Document) {
        if let Ok(named) = named(path) {
            self.inner().written.put(named, (key, document), WRITTEN);
        }
    }

    /// What the check `key` of the plate `held` names found before, or what `check` finds now.
    pub(crate) fn check<E>(
        &self,
        (key, held): (Key, &[ModelMeshes]),
        check: impl FnOnce() -> Result<Estimate, E>,
    ) -> Result<Estimate, E> {
        let kept = self.inner().checks.get(&key);
        if let Some(found) = kept.and_then(|(found, _)| found.finished_copy()) {
            return Ok(found);
        }
        let found = check()?;
        if let Some(kept) = found.finished_copy() {
            let entry = (Arc::new(kept), held.to_vec());
            self.inner().checks.put(key, entry, CHECKS);
        }
        Ok(found)
    }

    /// The plate this server wrote to the file at `path`, where the file is still as written,
    /// wherever it has been moved since.
    pub(crate) fn written(&self, path: &Path) -> Option<(Key, Document)> {
        let named = named(path).ok()?;
        self.inner().written.get(&named)
    }
}

/// What a model is made from: the object's state, its meshes by address and the support groups
/// and raft its trees are meshed to; `None` for a state that does not serialise, kept by none.
fn object_key(object: &ObjectState, meshes: &ModelMeshes, plate: &SupportState) -> Option<Key> {
    let state = serde_json::to_vec(object).ok()?;
    let table = format!("{:?} {:?}", plate.groups, plate.raft);
    let shell = meshes.shell.as_ref().map(Arc::as_ptr);
    let at = format!("{:p} {shell:?}", Arc::as_ptr(&meshes.source));
    Some(Key::of(&[
        b"model",
        &state,
        table.as_bytes(),
        at.as_bytes(),
    ]))
}

/// A plate's models by key, with the meshes it names by address, which whoever keeps the key
/// holds so no other mesh takes an address while it is kept.
#[derive(Clone)]
pub(crate) struct Fingerprint {
    pub key: Key,
    pub held: Vec<ModelMeshes>,
}

/// The key of a plate's `objects` as a run cuts them, their supports to `plate`'s groups.
pub(crate) fn plate_key<'a>(
    objects: impl IntoIterator<Item = (&'a ObjectState, &'a ModelMeshes)>,
    plate: &SupportState,
) -> Option<Fingerprint> {
    let mut key = Key::of(&[b"plate"]);
    let mut held = Vec::new();
    for (object, meshes) in objects {
        key = key.with(object_key(object, meshes, plate)?);
        held.push(meshes.clone());
    }
    Some(Fingerprint { key, held })
}

/// The key of an edit carried out: what it asks, and the bytes of any file it reads.
pub(crate) fn edit_bytes(edit: &Edit) -> Vec<u8> {
    let mut bytes = format!("{edit:?}").into_bytes();
    let read = match edit {
        Edit::Label {
            font: Some(path), ..
        }
        | Edit::Relief { image: path, .. } => Some(path),
        _ => None,
    };
    if let Some(contents) = read.and_then(|path| std::fs::read(path).ok()) {
        bytes.extend_from_slice(&Key::of(&[&contents]).0);
    }
    bytes
}

fn named(path: &Path) -> std::io::Result<Named> {
    let metadata = std::fs::metadata(path)?;
    let modified_ns = modified_ns(&metadata);
    let len = metadata.len();
    let mut file = std::fs::File::open(path)?;
    let mut head = Vec::new();
    (&mut file).take(SAMPLED).read_to_end(&mut head)?;
    let mut tail = Vec::new();
    file.seek(SeekFrom::Start(len.saturating_sub(SAMPLED)))?;
    file.take(SAMPLED).read_to_end(&mut tail)?;
    Ok(Named {
        len,
        modified_ns,
        ends: Key::of(&[&head, &tail]),
    })
}

fn modified_ns(metadata: &Metadata) -> u128 {
    (metadata.modified().ok())
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |since| since.as_nanos())
}

#[cfg(test)]
mod tests;
