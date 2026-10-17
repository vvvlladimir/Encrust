//! A resin another program exported as a profile of its own, read into one of ours. What
//! each format holds and where is in `docs/formats/resin-profiles.md`.

mod cfgx;
mod material_ini;

use std::path::Path;

use crate::{MaterialProfile, ProfileError};

/// A resin another program exported, and the machine it was set up for there.
#[derive(Debug, Clone, PartialEq)]
pub struct ForeignResin {
    /// The resin, on our defaults wherever the file has no place for a setting.
    pub resin: MaterialProfile,
    /// The machine the file names, where it names one.
    pub machine: Option<String>,
    /// That machine's panel in pixels, where the file states it.
    pub panel_px: Option<[u32; 2]>,
}

impl ForeignResin {
    /// Whether `path` is named as a profile this reads: a `.cfgx` or an `.ini`.
    pub fn reads(path: &Path) -> bool {
        Format::of(path).is_some()
    }

    /// Reads `text` as the profile its `path` names, a resin named after the file where
    /// the profile names none.
    pub fn from_str(text: &str, path: &Path) -> Result<Self, ProfileError> {
        let mut foreign = match Format::of(path) {
            Some(Format::Cfgx) => cfgx::read(text, path)?,
            Some(Format::MaterialIni) => material_ini::read(text, path)?,
            None => {
                return Err(ProfileError::UnknownFormat {
                    path: path.to_owned(),
                });
            }
        };
        if foreign.resin.name.trim().is_empty() {
            foreign.resin.name = path
                .file_stem()
                .map_or_else(|| "Resin".to_owned(), |stem| stem.to_string_lossy().into());
        }
        Ok(foreign)
    }
}

enum Format {
    Cfgx,
    MaterialIni,
}

impl Format {
    fn of(path: &Path) -> Option<Self> {
        let extension = path.extension()?.to_str()?.to_ascii_lowercase();
        match extension.as_str() {
            "cfgx" => Some(Self::Cfgx),
            "ini" => Some(Self::MaterialIni),
            _ => None,
        }
    }
}

/// The name a file gives, or `None` where it is blank.
fn named(name: &str) -> Option<String> {
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_owned())
}
