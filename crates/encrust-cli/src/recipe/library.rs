//! The recipes by id as `core_recipe::library` lists and finds them: the built-in ones, then
//! the user's `<stem>.toml` files the catalogue keeps in `recipes/` (ADR 0403, 0405).

use std::fmt;
use std::path::Path;

use anyhow::{Context, Result, bail};
use core_recipe::{BUILT_IN, Origin, Read, Recipe};
use printer_profiles::{Kind, is_valid_id, user_dir};
use serde::Serialize;

/// Where a recipe was found.
#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    BuiltIn,
    User,
    File,
}

impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Self::BuiltIn => "built-in",
            Self::User => "user",
            Self::File => "file",
        }
    }
}

/// One recipe of `recipes list`: its id, its name, or why it would not read.
#[derive(Serialize, Debug)]
pub struct Line {
    pub id: String,
    pub name: Option<String>,
    pub source: Source,
    pub error: Option<String>,
}

/// The recipes `recipes list` prints, as JSON and as text.
#[derive(Serialize, Debug)]
pub struct Listing {
    pub recipes: Vec<Line>,
}

impl fmt::Display for Listing {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let width = self.recipes.iter().map(|line| line.id.len()).max();
        for line in &self.recipes {
            let said = match (&line.name, &line.error) {
                (_, Some(error)) => format!("does not read: {error}"),
                (Some(name), None) => name.clone(),
                (None, None) => String::new(),
            };
            let user = if line.source == Source::User {
                "  [user]"
            } else {
                ""
            };
            let width = width.unwrap_or(0);
            writeln!(out, "{:<width$}  {said}{user}", line.id)?;
        }
        Ok(())
    }
}

/// The user's recipes as the catalogue keeps them: the `recipes/` folder beside their
/// profiles, the same place the window writes to (ADR 0405).
fn kept() -> Result<Vec<(String, String)>> {
    let catalogue = crate::profiles::catalogue()?;
    Ok(catalogue
        .recipes()
        .map(|entry| (entry.id.clone(), entry.profile.clone()))
        .collect())
}

fn pairs(kept: &[(String, String)]) -> impl Iterator<Item = (&str, &str)> {
    kept.iter().map(|(id, text)| (id.as_str(), text.as_str()))
}

/// Every built-in recipe, then the user's by stem; one that would not read is listed with
/// its error.
pub fn list() -> Result<Listing> {
    let kept = kept()?;
    let recipes = core_recipe::library(pairs(&kept))
        .into_iter()
        .map(|listed| {
            let error = listed.plan().err().map(ToString::to_string);
            Line {
                name: listed.recipe().map(|recipe| recipe.name.clone()),
                id: listed.id,
                source: match listed.origin {
                    Origin::BuiltIn => Source::BuiltIn,
                    Origin::User => Source::User,
                },
                error,
            }
        })
        .collect();
    Ok(Listing { recipes })
}

/// The recipe `asked` names, and where it was found: a built-in id, else the user's recipe
/// of that stem, or a recipe file by its path.
pub fn find(asked: &str) -> Result<(Source, Recipe)> {
    let path = Path::new(asked);
    if path.is_file() || path.extension().is_some_and(|ext| ext == "toml") {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read the recipe {}", path.display()))?;
        let recipe = Recipe::from_toml(&text)
            .with_context(|| format!("{} is not a recipe this build reads", path.display()))?;
        return Ok((Source::File, recipe));
    }
    let kept = if is_valid_id(asked) {
        kept()?
    } else {
        Vec::new()
    };
    if let Some(listed) = core_recipe::find(asked, pairs(&kept)) {
        let source = match listed.origin {
            Origin::BuiltIn => Source::BuiltIn,
            Origin::User => Source::User,
        };
        return match listed.read {
            Read::Valid(plan) => Ok((source, plan.recipe().clone())),
            Read::Invalid { recipe, .. } => Ok((source, recipe)),
            Read::Unreadable(error) => Err(anyhow::Error::new(error)
                .context(format!("{asked} is not a recipe this build reads"))),
        };
    }
    let ids: Vec<&str> = BUILT_IN.iter().map(|(id, _)| *id).collect();
    let folder = user_dir().map(|dir| dir.join(Kind::Recipe.dir()));
    bail!(
        "no recipe `{asked}`: the built-in ones are {}; yours are the files in {}; \
         `encrust recipes list` shows them",
        ids.join(", "),
        folder.map_or_else(|| "recipes/".to_owned(), |dir| dir.display().to_string())
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_built_in_recipe_is_listed_first_and_reads() {
        let listing = list().expect("the shipped catalogue reads");
        let built_in: Vec<_> = listing
            .recipes
            .iter()
            .take(BUILT_IN.len())
            .map(|line| (line.source, line.error.as_deref()))
            .collect();
        assert!(built_in.iter().all(|got| *got == (Source::BuiltIn, None)));
    }
}
