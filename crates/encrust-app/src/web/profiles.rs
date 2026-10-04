//! The user's own profiles in a browser: one entry of the page's storage each, keyed the
//! way a directory would name the file.

use std::path::PathBuf;

use printer_profiles::{Kind, ProfileError, ProfileStore};

const PREFIX: &str = "encrust.profiles/";

/// The page's storage as a profile directory. It holds nothing of the page itself, so a
/// catalogue cloned onto a worker still carries it; only the page's thread reaches it.
#[derive(Debug)]
pub struct PageProfiles;

impl ProfileStore for PageProfiles {
    fn path_of(&self, kind: Kind, id: &str) -> PathBuf {
        PathBuf::from(format!("{}/{id}.toml", kind.dir()))
    }

    fn read_all(&self, kind: Kind) -> Result<Vec<(String, PathBuf, String)>, ProfileError> {
        let Some(storage) = storage() else {
            return Ok(Vec::new());
        };
        let folder = format!("{PREFIX}{}/", kind.dir());
        let length = storage.length().unwrap_or(0);
        let mut read = Vec::new();
        for index in 0..length {
            let Some(key) = storage.key(index).ok().flatten() else {
                continue;
            };
            let Some(id) = key
                .strip_prefix(&folder)
                .and_then(|name| name.strip_suffix(".toml"))
            else {
                continue;
            };
            if let Some(toml) = storage.get_item(&key).ok().flatten() {
                read.push((id.to_owned(), self.path_of(kind, id), toml));
            }
        }
        Ok(read)
    }

    fn write(&self, kind: Kind, id: &str, toml: &str) -> Result<(), ProfileError> {
        let path = self.path_of(kind, id);
        let storage = storage().ok_or_else(|| refused(&path, "no page storage here"))?;
        storage
            .set_item(&key(&path), toml)
            .map_err(|_| refused(&path, "the page's storage is full or switched off"))
    }

    fn remove(&self, kind: Kind, id: &str) -> Result<(), ProfileError> {
        let path = self.path_of(kind, id);
        if let Some(storage) = storage() {
            let _ = storage.remove_item(&key(&path));
        }
        Ok(())
    }
}

fn key(path: &std::path::Path) -> String {
    format!("{PREFIX}{}", path.display())
}

fn refused(path: &std::path::Path, why: &str) -> ProfileError {
    ProfileError::Io {
        path: path.to_owned(),
        source: std::io::Error::other(why),
    }
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}
