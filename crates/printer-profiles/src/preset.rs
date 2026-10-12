use std::path::Path;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::ProfileError;

/// A named set of a tool's settings, kept as TOML whose shape is the tool's own: this crate
/// stores and lists it, and the tool that saved it reads it back. See
/// `docs/decisions/0226`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub settings: toml::Table,
}

impl Preset {
    /// `settings` under `name`, as the table it is written as.
    pub fn of<T: Serialize>(name: &str, settings: &T, path: &Path) -> Result<Self, ProfileError> {
        let settings =
            toml::Table::try_from(settings).map_err(|source| ProfileError::Serialise {
                path: path.to_owned(),
                source,
            })?;
        Ok(Self {
            name: name.to_owned(),
            settings,
        })
    }

    /// The settings read back as the tool's own type.
    pub fn read<T: DeserializeOwned>(&self, path: &Path) -> Result<T, ProfileError> {
        self.settings
            .clone()
            .try_into()
            .map_err(|source| ProfileError::Parse {
                path: path.to_owned(),
                source,
            })
    }

    pub fn to_toml_string(&self, path: &Path) -> Result<String, ProfileError> {
        toml::to_string_pretty(self).map_err(|source| ProfileError::Serialise {
            path: path.to_owned(),
            source,
        })
    }

    pub fn from_toml_str(source: &str, path: &Path) -> Result<Self, ProfileError> {
        toml::from_str(source).map_err(|source| ProfileError::Parse {
            path: path.to_owned(),
            source,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Wall {
        thickness_mm: f32,
        infill: bool,
    }

    #[test]
    fn a_tools_settings_come_back_as_they_were_saved() {
        let path = Path::new("hollow/hive.toml");
        let wall = Wall {
            thickness_mm: 2.0,
            infill: true,
        };
        let preset = Preset::of("Hive 2 mm", &wall, path).expect("a struct is a table");
        let text = preset.to_toml_string(path).expect("a table writes");
        let read = Preset::from_toml_str(&text, path).expect("what was written reads");
        assert_eq!(read.name, "Hive 2 mm");
        assert_eq!(read.read::<Wall>(path).expect("the same shape"), wall);
    }

    #[test]
    fn settings_of_another_shape_are_a_parse_error() {
        let path = Path::new("hollow/odd.toml");
        let preset = Preset::from_toml_str("name = \"Odd\"\n[settings]\nwall = \"thick\"\n", path)
            .expect("any table reads as a preset");
        assert!(matches!(
            preset.read::<Wall>(path),
            Err(ProfileError::Parse { .. })
        ));
    }
}
