//! The models a run prepares, read from a TOML list so a corpus kept outside the repository
//! carries its own paths.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

/// One model of a corpus.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// The file's stem unless the corpus names it.
    pub name: String,
    /// Resolved against the corpus file's own directory.
    pub path: PathBuf,
    /// Why the model is listed but not prepared.
    pub skip: Option<String>,
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
    skip: Option<String>,
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
            skip: raw.skip,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_model_is_named_by_its_stem_and_found_beside_the_corpus() {
        let entries = parse(
            "[[model]]\npath = \"prints/bust.stl\"\n\
             [[model]]\nname = \"big\"\npath = \"/abs/x.obj\"\nskip = \"too slow\"\n",
            Path::new("/bench"),
        )
        .expect("the corpus is well formed");
        assert_eq!(entries[0].name, "bust");
        assert_eq!(entries[0].path, Path::new("/bench/prints/bust.stl"));
        assert_eq!(entries[1].name, "big");
        assert_eq!(entries[1].path, Path::new("/abs/x.obj"));
        assert_eq!(entries[1].skip.as_deref(), Some("too slow"));
    }

    #[test]
    fn a_misspelt_field_is_refused() {
        assert!(
            parse(
                "[[model]]\npath = \"a.stl\"\nskipped = \"x\"\n",
                Path::new(".")
            )
            .is_err()
        );
    }
}
