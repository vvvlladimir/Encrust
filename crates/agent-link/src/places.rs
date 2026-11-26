//! Where the server and the window meet: folders and a file under the user's cache.

use std::path::PathBuf;

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
