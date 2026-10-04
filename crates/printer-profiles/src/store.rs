//! Where the user's own profiles are kept: a directory at the desk, the page's storage in a
//! browser, which has no directory to give. See docs/decisions/0181.

use std::path::PathBuf;

use crate::{Kind, ProfileError};

/// The user's half of the catalogue, read whole and written one profile at a time.
pub trait ProfileStore: std::fmt::Debug + Send + Sync {
    /// Where the profile of `kind` and `id` is, or would be, kept: what an error names and
    /// what the catalogue entry points at.
    fn path_of(&self, kind: Kind, id: &str) -> PathBuf;

    /// Every profile of `kind` kept, as its id, where it is kept and its TOML.
    fn read_all(&self, kind: Kind) -> Result<Vec<(String, PathBuf, String)>, ProfileError>;

    fn write(&self, kind: Kind, id: &str, toml: &str) -> Result<(), ProfileError>;

    /// Throws the profile away. One that was never kept is not an error.
    fn remove(&self, kind: Kind, id: &str) -> Result<(), ProfileError>;
}

/// A directory of `printers/`, `resins/` and `supports/`, one TOML file per profile.
#[derive(Debug, Clone)]
pub struct DirStore {
    root: PathBuf,
}

impl DirStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl ProfileStore for DirStore {
    fn path_of(&self, kind: Kind, id: &str) -> PathBuf {
        self.root.join(kind.dir()).join(format!("{id}.toml"))
    }

    fn read_all(&self, kind: Kind) -> Result<Vec<(String, PathBuf, String)>, ProfileError> {
        let dir = self.root.join(kind.dir());
        // A missing directory is not an error: most users have never made one.
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => return Err(ProfileError::Io { path: dir, source }),
        };
        let mut read = Vec::new();
        for entry in entries {
            let path = entry
                .map_err(|source| ProfileError::Io {
                    path: dir.clone(),
                    source,
                })?
                .path();
            if path.extension().is_none_or(|extension| extension != "toml") {
                continue;
            }
            let Some(id) = path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            let toml = std::fs::read_to_string(&path).map_err(|source| ProfileError::Io {
                path: path.clone(),
                source,
            })?;
            read.push((id.to_owned(), path, toml));
        }
        Ok(read)
    }

    /// Makes the directory first: it does not exist until the first profile is saved.
    fn write(&self, kind: Kind, id: &str, toml: &str) -> Result<(), ProfileError> {
        let path = self.path_of(kind, id);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|source| ProfileError::Io {
                path: dir.to_owned(),
                source,
            })?;
        }
        std::fs::write(&path, toml).map_err(|source| ProfileError::Io { path, source })
    }

    fn remove(&self, kind: Kind, id: &str) -> Result<(), ProfileError> {
        let path = self.path_of(kind, id);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(ProfileError::Io { path, source }),
        }
    }
}
