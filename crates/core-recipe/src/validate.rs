//! A recipe checked whole before it runs: its names, when each is measured, the kinds of its
//! conditions, and the notes on steps that undo earlier ones.

use crate::error::RecipeError;
use crate::recipe::{Recipe, StepKind};

/// A later step that undoes an earlier one, as the Order table of `docs/design/recipes.md`
/// lists; the plan shows it under step `step`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Note {
    pub step: usize,
    pub kind: StepKind,
    /// The step it undoes, by place and kind.
    pub over: usize,
    pub over_kind: StepKind,
}

/// A recipe `validate` passed, the only kind `decide` takes.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    recipe: Recipe,
    notes: Vec<Note>,
}

impl Plan {
    pub fn recipe(&self) -> &Recipe {
        &self.recipe
    }

    pub fn notes(&self) -> &[Note] {
        &self.notes
    }
}

/// Checks every name a condition reads and that a step before measures it, every
/// condition's kind and each retry's bound, and notes the steps that undo earlier ones.
pub fn validate(recipe: Recipe) -> Result<Plan, RecipeError> {
    // TODO(step-2): kinds, names and their timing (`Metric::known_after`), the notes' table.
    drop(recipe);
    todo!()
}
