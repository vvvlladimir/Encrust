//! Where a run keeps the orientation searches it made: this build's folder under the user's
//! cache, which the window built with it shares (ADR 0466).

use std::path::PathBuf;
use std::sync::LazyLock;

static FOLDER: LazyLock<Option<PathBuf>> = LazyLock::new(|| {
    let dirs = directories::ProjectDirs::from("", "", "Encrust");
    core_engine::kept_poses_folder(dirs.as_ref().map(directories::ProjectDirs::cache_dir))
});

/// The folder this run keeps searches in, none for none.
pub(crate) fn folder() -> Option<PathBuf> {
    FOLDER.clone()
}
