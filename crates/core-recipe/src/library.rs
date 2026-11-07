//! The recipes by id, as both front ends list and find them: the built-in ones first, then
//! the user's by stem, a built-in id winning over a user's file of the same stem. The front
//! ends read the user's files and hand their text in (ADR 0405).

use crate::builtin::BUILT_IN;
use crate::error::RecipeError;
use crate::recipe::Recipe;
use crate::validate::{Plan, validate};

#[cfg(test)]
mod tests;

/// Where a recipe of the library comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    BuiltIn,
    User,
}

/// A recipe's text as read: checked and ready to run, read but refused by `validate`, or not
/// a recipe this build reads.
#[derive(Debug, Clone, PartialEq)]
pub enum Read {
    Valid(Plan),
    Invalid { recipe: Recipe, error: RecipeError },
    Unreadable(RecipeError),
}

impl Read {
    /// `text` read as a recipe and checked.
    pub fn of(text: &str) -> Self {
        match Recipe::from_toml(text) {
            Ok(recipe) => match validate(recipe.clone()) {
                Ok(plan) => Self::Valid(plan),
                Err(error) => Self::Invalid { recipe, error },
            },
            Err(error) => Self::Unreadable(error),
        }
    }
}

/// One recipe of the library under its id.
#[derive(Debug, Clone, PartialEq)]
pub struct Listed {
    pub id: String,
    pub origin: Origin,
    pub read: Read,
}

impl Listed {
    /// The recipe as written, when it reads at all.
    pub fn recipe(&self) -> Option<&Recipe> {
        match &self.read {
            Read::Valid(plan) => Some(plan.recipe()),
            Read::Invalid { recipe, .. } => Some(recipe),
            Read::Unreadable(_) => None,
        }
    }

    /// What the picker shows: its name, else its id.
    pub fn name(&self) -> &str {
        self.recipe().map_or(&self.id, |recipe| &recipe.name)
    }

    /// The plan to run, or why there is none.
    pub fn plan(&self) -> Result<&Plan, &RecipeError> {
        match &self.read {
            Read::Valid(plan) => Ok(plan),
            Read::Invalid { error, .. } | Read::Unreadable(error) => Err(error),
        }
    }

    /// A user's recipe a built-in one of the same id hides from a lookup by id.
    pub fn is_shadowed(&self) -> bool {
        self.origin == Origin::User && is_built_in(&self.id)
    }
}

/// Whether a recipe ships under `id`.
pub fn is_built_in(id: &str) -> bool {
    BUILT_IN.iter().any(|(name, _)| *name == id)
}

/// Every built-in recipe, then each of `user`, `(id, text)`, sorted by id.
pub fn library<'a>(user: impl IntoIterator<Item = (&'a str, &'a str)>) -> Vec<Listed> {
    let mut listed: Vec<Listed> = BUILT_IN
        .iter()
        .map(|(id, text)| entry(id, Origin::BuiltIn, text))
        .collect();
    let mut users: Vec<(&str, &str)> = user.into_iter().collect();
    users.sort_unstable();
    listed.extend(
        users
            .into_iter()
            .map(|(id, text)| entry(id, Origin::User, text)),
    );
    listed
}

/// The recipe `id` names among the built-in ones, else among `user`'s.
pub fn find<'a>(id: &str, user: impl IntoIterator<Item = (&'a str, &'a str)>) -> Option<Listed> {
    if let Some((_, text)) = BUILT_IN.iter().find(|(name, _)| *name == id) {
        return Some(entry(id, Origin::BuiltIn, text));
    }
    user.into_iter()
        .find(|(name, _)| *name == id)
        .map(|(_, text)| entry(id, Origin::User, text))
}

fn entry(id: &str, origin: Origin, text: &str) -> Listed {
    Listed {
        id: id.to_owned(),
        origin,
        read: Read::of(text),
    }
}
