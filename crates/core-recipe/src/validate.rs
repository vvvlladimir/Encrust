//! A recipe checked whole before it runs: its order, its names and the kinds of its conditions.

use crate::error::RecipeError;
use crate::recipe::Recipe;

/// A recipe `validate` passed, the only kind `decide` takes.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    recipe: Recipe,
}

impl Plan {
    pub fn recipe(&self) -> &Recipe {
        &self.recipe
    }
}

/// Checks the order of docs/design/recipes.md, every name a condition reads and when it is
/// measured, every condition's kind, and each retry's bound.
pub fn validate(recipe: Recipe) -> Result<Plan, RecipeError> {
    // TODO(step-2): the order table and kinds; the names' timing once Metric has it (step-3).
    drop(recipe);
    todo!()
}
