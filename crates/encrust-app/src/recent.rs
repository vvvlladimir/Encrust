//! The files opened or saved lately, newest first, which the start page offers again.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// How many files are remembered: more than the start page shows, so one that has gone
/// missing does not leave it short.
const KEPT: usize = 8;

/// One file, and when it was last opened or saved.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentFile {
    pub path: PathBuf,
    /// Seconds since the Unix epoch.
    pub opened_at_s: u64,
}

impl RecentFile {
    /// What the start page calls it: the file's name without its folder.
    pub fn name(&self) -> String {
        self.path.file_name().map_or_else(
            || self.path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        )
    }
}

/// The files, newest first, each once.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Recent(Vec<RecentFile>);

impl Recent {
    /// Puts `path` first. A browser hands over bytes under a name, never a path it can
    /// open again, so there nothing is kept.
    pub fn note(&mut self, path: &Path, now_s: u64) {
        if cfg!(target_arch = "wasm32") {
            return;
        }
        self.forget(path);
        self.0.insert(
            0,
            RecentFile {
                path: path.to_path_buf(),
                opened_at_s: now_s,
            },
        );
        self.0.truncate(KEPT);
    }

    /// Drops `path`, which is what a file that cannot be opened any more gets.
    pub fn forget(&mut self, path: &Path) {
        self.0.retain(|file| file.path != path);
    }

    pub fn files(&self) -> &[RecentFile] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_opened_again_moves_to_the_front_once() {
        let mut recent = Recent::default();
        recent.note(Path::new("/a.stl"), 1);
        recent.note(Path::new("/b.encrust"), 2);
        recent.note(Path::new("/a.stl"), 3);
        let paths: Vec<&Path> = recent
            .files()
            .iter()
            .map(|file| file.path.as_path())
            .collect();
        assert_eq!(paths, [Path::new("/a.stl"), Path::new("/b.encrust")]);
        assert_eq!(recent.files()[0].opened_at_s, 3);
    }

    #[test]
    fn only_the_newest_are_kept() {
        let mut recent = Recent::default();
        for index in 0..KEPT + 3 {
            recent.note(&PathBuf::from(format!("/{index}.stl")), index as u64);
        }
        assert_eq!(recent.files().len(), KEPT);
        assert_eq!(recent.files()[0].name(), format!("{}.stl", KEPT + 2));
    }

    #[test]
    fn a_forgotten_file_is_gone() {
        let mut recent = Recent::default();
        recent.note(Path::new("/a.stl"), 1);
        recent.forget(Path::new("/a.stl"));
        assert!(recent.files().is_empty());
    }
}
