//! The models a run benches, how each is stood for printing, and what a person expects of
//! it, read from a TOML file so a corpus kept outside the repository carries all three.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

/// One model of a corpus.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// What the report calls it: the file's stem unless the corpus names it, as it must to
    /// list one file in two poses.
    pub name: String,
    /// Resolved against the corpus file's own directory.
    pub path: PathBuf,
    pub note: String,
    /// Why the model is listed but not benched.
    pub skip: Option<String>,
    pub pose: Pose,
    pub expect: Expect,
}

/// How a model is stood before it is supported: which of its axes points up, how tall it
/// prints and how far it leans, the way a person sets a figure up by hand.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Pose {
    #[serde(default)]
    pub up: Up,
    /// The model is scaled to stand this tall before it leans, millimetres; as drawn if
    /// unset.
    pub height_mm: Option<f32>,
    /// Lean about the plate's X axis, degrees, after scaling.
    #[serde(default)]
    pub tilt_deg: f32,
    /// Turn about the vertical, degrees, before the lean: which side leans away.
    #[serde(default)]
    pub spin_deg: f32,
}

impl Default for Pose {
    fn default() -> Self {
        Self {
            up: Up::Z,
            height_mm: None,
            tilt_deg: 0.0,
            spin_deg: 0.0,
        }
    }
}

/// The axis of the file that points up.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Up {
    #[default]
    Z,
    Y,
    NegY,
    X,
}

/// What a person expects of the model's supports: limits a profile's result is held to on
/// top of the rules every model shares.
#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Expect {
    /// Most contacts a person would put under it.
    pub max_contacts: Option<usize>,
    /// Most resin in its supports, millilitres.
    pub max_supports_ml: Option<f32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    model: Vec<Raw>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    name: Option<String>,
    path: PathBuf,
    #[serde(default)]
    note: String,
    skip: Option<String>,
    #[serde(default)]
    pose: Pose,
    #[serde(default)]
    expect: Expect,
}

/// Reads the corpus at `path`.
pub fn read(path: &Path) -> Result<Vec<Entry>> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    let base = path.parent().unwrap_or(Path::new("."));
    parse(&text, base).with_context(|| format!("cannot read the corpus {}", path.display()))
}

fn parse(text: &str, base: &Path) -> Result<Vec<Entry>> {
    let file: File = toml::from_str(text)?;
    Ok(file
        .model
        .into_iter()
        .map(|raw| Entry {
            name: raw.name.unwrap_or_else(|| crate::name_of(&raw.path)),
            path: base.join(raw.path),
            note: raw.note,
            skip: raw.skip,
            pose: raw.pose,
            expect: raw.expect,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pose_and_expectations_are_read_with_paths_resolved() {
        let entries = parse(
            r#"
            [[model]]
            path = "models/knight.obj"
            note = "a figure"
            pose = { up = "y", height-mm = 32, tilt-deg = 30 }
            expect = { max-contacts = 60 }
            "#,
            Path::new("/bench"),
        )
        .expect("the corpus is well formed");
        let entry = &entries[0];
        assert_eq!(entry.path, Path::new("/bench/models/knight.obj"));
        assert_eq!(entry.name, "knight");
        assert_eq!(entry.pose.up, Up::Y);
        assert_eq!(entry.pose.height_mm, Some(32.0));
        assert_eq!(entry.expect.max_contacts, Some(60));
        assert_eq!(entry.expect.max_supports_ml, None);
    }

    #[test]
    fn a_misspelt_field_is_refused() {
        let parsed = parse(
            "[[model]]\npath = \"a.stl\"\npose = { height = 30 }\n",
            Path::new("."),
        );
        assert!(
            parsed.is_err(),
            "a typo must not silently stand the model as drawn"
        );
    }
}
