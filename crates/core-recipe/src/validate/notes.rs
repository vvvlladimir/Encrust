//! The Order table of `docs/design/recipes.md`: which later step undoes which earlier one.

use super::Note;
use crate::recipe::{Recipe, Setting, Step, StepKind};

pub(super) fn notes(recipe: &Recipe) -> Vec<Note> {
    let mut notes = Vec::new();
    for (step, later) in recipe.steps.iter().enumerate().filter(|(_, s)| s.enabled) {
        for (over, earlier) in recipe.steps[..step]
            .iter()
            .enumerate()
            .filter(|(_, s)| s.enabled)
        {
            if undoes(later.kind, earlier) {
                notes.push(Note {
                    step,
                    kind: later.kind,
                    over,
                    over_kind: earlier.kind,
                });
            }
        }
    }
    notes
}

fn undoes(later: StepKind, earlier: &Step) -> bool {
    use StepKind::{Drain, Hollow, Orient, Repair, Supports};
    match (later, earlier.kind) {
        (Orient | Hollow | Supports, Supports) | (Repair, Hollow | Drain) => true,
        (Orient, Drain) => {
            !matches!(earlier.set.get("at"), Some(Setting::Word(w)) if w == "points")
        }
        _ => false,
    }
}
