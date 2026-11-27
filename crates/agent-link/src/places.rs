//! Where the server and the window meet: folders and a file under the user's cache.

use std::path::{Path, PathBuf};

/// The folder of the server's jobs, one folder a job.
pub const JOBS: &str = "mcp-jobs";
/// The folder of the servers' own files, one a server, which the window lists.
pub const AGENTS: &str = "agents";
/// The file the open window writes to say where it listens.
pub const WINDOW_FILE: &str = "window.json";

/// Encrust's folder in the user's cache, which holds the three above; `None` where the system
/// names none.
pub fn cache_dir() -> Option<PathBuf> {
    let dirs = directories::ProjectDirs::from("", "", "Encrust")?;
    Some(dirs.cache_dir().to_owned())
}

/// One of the files a job's folder keeps for each revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevisionFile {
    /// The server's recipe, `N.toml`.
    Recipe,
    /// Its check's report, `N.json`.
    Check,
    /// The flags it stages with, the model first, `N.args.json`.
    Args,
    /// The edits it handed the window, `N.edits.json`.
    Edits,
    /// The plate its check wrote, `N.encrust`.
    Plate,
}

impl RevisionFile {
    /// Its name for revision `number`.
    pub fn of(self, number: u32) -> String {
        let extension = match self {
            Self::Recipe => "toml",
            Self::Check => "json",
            Self::Args => "args.json",
            Self::Edits => "edits.json",
            Self::Plate => "encrust",
        };
        format!("{number}.{extension}")
    }
}

/// Whether `job` names a job's folder and nothing outside it: letters, digits, `-` and `_`.
pub fn is_job_name(job: &str) -> bool {
    !job.is_empty()
        && job
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// The number of the revision `path` is the recipe of, `None` for any other file.
pub fn revision_of(path: &Path) -> Option<u32> {
    if path.extension()? != "toml" {
        return None;
    }
    path.file_stem()?.to_str()?.parse().ok()
}

/// The latest revision kept in the job's folder `dir`, its highest recipe; `None` with none.
pub fn latest_revision(dir: &Path) -> Option<u32> {
    (std::fs::read_dir(dir).ok()?)
        .filter_map(|entry| revision_of(&entry.ok()?.path()))
        .max()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_job_name_reaches_no_folder_but_its_own() {
        assert!(is_job_name("bust-final_2"));
        for outside in ["", "..", "a/b", "a\\b", "."] {
            assert!(!is_job_name(outside), "{outside:?}");
        }
    }

    #[test]
    fn only_a_revisions_recipe_numbers_it() {
        assert_eq!(revision_of(Path::new("/j/12.toml")), Some(12));
        assert_eq!(revision_of(Path::new("/j/12.recipe.toml")), None);
        assert_eq!(revision_of(Path::new("/j/12.json")), None);
        assert_eq!(RevisionFile::Edits.of(3), "3.edits.json");
    }
}
