//! The recipes by id: the built-in ones, then the user's `<stem>.toml` files in the
//! `recipes/` folder beside their profiles (ADR 0403).

use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use core_recipe::{BUILT_IN, Recipe, validate};
use printer_profiles::{is_valid_id, user_dir};
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

/// The folder of the user's recipes, which may not exist.
fn user_folder() -> Option<PathBuf> {
    user_dir().map(|dir| dir.join("recipes"))
}

/// The file of the user's recipe `id`, when there is one.
fn user_file(id: &str) -> Option<PathBuf> {
    if !is_valid_id(id) {
        return None;
    }
    let path = user_folder()?.join(format!("{id}.toml"));
    path.is_file().then_some(path)
}

/// Every built-in recipe, then the user's by stem; one that would not read is listed with
/// its error.
pub fn list() -> Listing {
    let mut recipes: Vec<Line> = BUILT_IN
        .iter()
        .map(|(id, text)| line(id, Source::BuiltIn, read_text(text)))
        .collect();
    let mut users: Vec<(String, PathBuf)> = user_folder()
        .and_then(|folder| std::fs::read_dir(folder).ok())
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            let stem = path.file_stem()?.to_str()?.to_owned();
            (path.extension()? == "toml").then_some((stem, path))
        })
        .collect();
    users.sort();
    recipes.extend(
        users
            .into_iter()
            .map(|(id, path)| line(&id, Source::User, read_file(&path))),
    );
    Listing { recipes }
}

fn line(id: &str, source: Source, read: Result<Recipe>) -> Line {
    let (name, error) = match read {
        Ok(recipe) => (Some(recipe.name), None),
        Err(error) => (None, Some(format!("{error:#}"))),
    };
    Line {
        id: id.to_owned(),
        name,
        source,
        error,
    }
}

fn read_text(text: &str) -> Result<Recipe> {
    let recipe = Recipe::from_toml(text)?;
    validate(recipe.clone())?;
    Ok(recipe)
}

fn read_file(path: &Path) -> Result<Recipe> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read the recipe {}", path.display()))?;
    read_text(&text)
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
    if let Some((_, text)) = BUILT_IN.iter().find(|(id, _)| *id == asked) {
        return Ok((Source::BuiltIn, Recipe::from_toml(text)?));
    }
    if let Some(path) = user_file(asked) {
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("cannot read the recipe {}", path.display()))?;
        let recipe = Recipe::from_toml(&text)
            .with_context(|| format!("{} is not a recipe this build reads", path.display()))?;
        return Ok((Source::User, recipe));
    }
    let ids: Vec<&str> = BUILT_IN.iter().map(|(id, _)| *id).collect();
    bail!(
        "no recipe `{asked}`: the built-in ones are {}; yours are the files in {}; \
         `encrust recipes list` shows them",
        ids.join(", "),
        user_folder().map_or_else(|| "recipes/".to_owned(), |dir| dir.display().to_string())
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_built_in_recipe_is_listed_first_and_reads() {
        let listing = list();
        let built_in: Vec<_> = listing
            .recipes
            .iter()
            .take(BUILT_IN.len())
            .map(|line| (line.source, line.error.as_deref()))
            .collect();
        assert!(built_in.iter().all(|got| *got == (Source::BuiltIn, None)));
    }
}
