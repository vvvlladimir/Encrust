//! The poses a search ranked, kept on disk under the hash of everything it was asked, so a
//! run asking the same again reads them rather than searching (ADR 0466).

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use core_geometry::Mesh;
use core_plate::{OrientSettings, Printed, Watch};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::EngineError;
use crate::trial::{Choice, Trial, orient_for_print};

mod folder;

pub use folder::{KEPT_POSES_ENV, kept_poses_folder};

/// The folder under a program's cache the poses are kept in, one per layout of the files.
pub const KEPT_POSES: &str = "poses/v1";
/// Searches kept at most; the least recently read go first past it.
const KEPT_AT_MOST: usize = 512;

#[derive(Serialize, Deserialize)]
struct Kept {
    engine: String,
    choices: Vec<Choice>,
}

/// [`orient_for_print`], its answer read from `folder` where the same search was kept there and
/// kept there otherwise. A folder that cannot be read or written is a search like any other.
pub fn orient_for_print_kept(
    folder: Option<&Path>,
    printed: &Printed,
    settings: &OrientSettings,
    trial: Option<&Trial>,
    watch: Watch<'_>,
) -> Result<Vec<Choice>, EngineError> {
    // What shows is read off the model by the search itself; given ready, it is not keyed.
    let Some(folder) = folder.filter(|_| printed.shows.is_none()) else {
        return orient_for_print(printed, settings, trial, watch);
    };
    let path = folder.join(format!("{}.json", key_of(printed, settings, trial)));
    if let Some(choices) = read(&path) {
        return Ok(choices);
    }
    let choices = orient_for_print(printed, settings, trial, watch)?;
    if let Err(error) = keep(folder, &path, &choices) {
        tracing::debug!(
            "the poses found were not kept in {}: {error}",
            folder.display()
        );
    }
    Ok(choices)
}

/// The hash of every argument of the search and of the engine that runs it, as hex.
fn key_of(printed: &Printed, settings: &OrientSettings, trial: Option<&Trial>) -> String {
    let mut hash = Sha256::new();
    hash.update(env!("CARGO_PKG_VERSION"));
    hash_mesh(&mut hash, Some(printed.outer));
    hash_mesh(&mut hash, printed.shell);
    hash_mesh(&mut hash, printed.cuts);
    // Debug writes every field, a float as the shortest text that reads back to it.
    let asked = format!(
        "{:?}|{:?}|{settings:?}|{:?}",
        printed.vents,
        printed.shown,
        // The raster window only batches the layers, so the window's and the command line's
        // searches, a thread count apart, read one another's.
        trial.map(|trial| (
            trial.printer,
            trial.material,
            trial.support,
            trial.keep_clear
        ))
    );
    hash.update(asked);
    hash.finalize()
        .iter()
        .fold(String::with_capacity(64), |mut hex, byte| {
            _ = write!(hex, "{byte:02x}");
            hex
        })
}

fn hash_mesh(hash: &mut Sha256, mesh: Option<&Mesh>) {
    let Some(mesh) = mesh else {
        hash.update([0]);
        return;
    };
    hash.update([1]);
    hash.update((mesh.vertices.len() as u64).to_le_bytes());
    for vertex in &mesh.vertices {
        for coordinate in vertex.to_array() {
            hash.update(coordinate.to_le_bytes());
        }
    }
    for face in &mesh.faces {
        for index in face {
            hash.update(index.to_le_bytes());
        }
    }
}

/// The choices kept at `path` by this engine, marked as read just now; `None` for none.
fn read(path: &Path) -> Option<Vec<Choice>> {
    let kept: Kept = serde_json::from_slice(&fs::read(path).ok()?).ok()?;
    if kept.engine != env!("CARGO_PKG_VERSION") {
        return None;
    }
    if let Ok(file) = fs::File::options().write(true).open(path) {
        _ = file.set_modified(SystemTime::now());
    }
    Some(kept.choices)
}

/// Writes `choices` beside `path` and renames them onto it, so no reader sees half a file,
/// then lets the least recently read go past `KEPT_AT_MOST`.
fn keep(folder: &Path, path: &Path, choices: &[Choice]) -> std::io::Result<()> {
    fs::create_dir_all(folder)?;
    let kept = Kept {
        engine: env!("CARGO_PKG_VERSION").to_owned(),
        choices: choices.to_vec(),
    };
    let text = serde_json::to_vec(&kept).map_err(std::io::Error::other)?;
    let writing = path.with_extension(format!("{}.part", std::process::id()));
    fs::write(&writing, text)?;
    fs::rename(&writing, path)?;
    trim(folder)
}

fn trim(folder: &Path) -> std::io::Result<()> {
    let mut kept: Vec<(SystemTime, PathBuf)> = fs::read_dir(folder)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .collect();
    if kept.len() <= KEPT_AT_MOST {
        return Ok(());
    }
    kept.sort_unstable();
    for (_, path) in &kept[..kept.len() - KEPT_AT_MOST] {
        _ = fs::remove_file(path);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
