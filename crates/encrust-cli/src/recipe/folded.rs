//! `--recipe-out`: the recipe a run takes a model through, each flag typed for it written into
//! the steps it shapes, so another front end runs the same steps (ADR 0461).

use core_recipe::{Plan, Recipe, Resolved, Setting, Step, StepKind};

use super::settings::{Settings, forced, typed_over};
use crate::stage::Shaping;

/// The steps a run with no recipe takes, in the order the command line takes them.
const FLAGS_ORDER: [StepKind; 5] = [
    StepKind::Repair,
    StepKind::Orient,
    StepKind::Hollow,
    StepKind::Drain,
    StepKind::Supports,
];

/// `plan`'s recipe, or the flags' steps alone, with `shaping` folded in: a step a flag forces runs
/// whatever its `when`, a setting a flag gives replaces the step's own and every choice's.
pub fn folded(plan: Option<&Plan>, shaping: &Shaping) -> Recipe {
    let Some(plan) = plan else {
        return of_flags(shaping);
    };
    let mut recipe = plan.recipe().clone();
    for step in &mut recipe.steps {
        fold(step, shaping);
    }
    recipe
}

/// A recipe of the steps the flags ask for alone, in the command line's order.
pub(super) fn of_flags(shaping: &Shaping) -> Recipe {
    let steps = FLAGS_ORDER
        .into_iter()
        .filter_map(|kind| {
            let mut step = bare(kind);
            fold(&mut step, shaping);
            let forced = forced(shaping).any(|(typed, _)| typed == kind);
            let drained = kind == StepKind::Drain && step.set.contains_key("at");
            (forced || drained).then_some(step)
        })
        .collect();
    Recipe {
        name: "The command line's flags".to_owned(),
        vars: Default::default(),
        steps,
    }
}

fn fold(step: &mut Step, shaping: &Shaping) {
    if forced(shaping).any(|(typed, _)| typed == step.kind) {
        step.when = None;
    }
    let typed = typed_over(step.kind, Settings::new(), shaping);
    for (key, value) in typed {
        for choice in &mut step.choose {
            choice.set.remove(&key);
        }
        step.set.insert(key, setting(value));
    }
}

fn bare(kind: StepKind) -> Step {
    Step {
        kind,
        enabled: true,
        when: None,
        set: Default::default(),
        choose: Vec::new(),
        on_fail: Default::default(),
        message: None,
        stop_if: None,
        retry: None,
    }
}

fn setting(value: Resolved) -> Setting {
    match value {
        Resolved::Number(number) => Setting::Number(number),
        Resolved::Bool(flag) => Setting::Bool(flag),
        Resolved::Word(word) => Setting::Word(word),
        Resolved::Text(text) => Setting::Text(text),
        Resolved::Points(points) => Setting::Points(points),
    }
}

#[cfg(test)]
mod tests;
