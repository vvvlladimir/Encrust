//! The recipes that ship, by id: `prepare` is the autopilot of ADR 0402.

use crate::error::RecipeError;
use crate::recipe::Recipe;
use crate::validate::{Plan, validate};

/// Each built-in recipe's id and text.
pub const BUILT_IN: [(&str, &str); 1] = [(
    "prepare",
    include_str!("../../../assets/recipes/prepare.toml"),
)];

/// The built-in recipe `id`, read and checked.
pub fn built_in(id: &str) -> Result<Plan, RecipeError> {
    let (_, text) = BUILT_IN
        .iter()
        .find(|(name, _)| *name == id)
        .ok_or_else(|| RecipeError::UnknownBuiltIn { id: id.to_owned() })?;
    validate(Recipe::from_toml(text)?)
}
