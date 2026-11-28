use clap::Parser;
use core_recipe::{Setting, StepKind, built_in, validate};

use super::folded;
use crate::args::JobArgs;
use crate::stage::Shaping;

#[derive(Parser)]
struct Typed {
    #[command(flatten)]
    job: JobArgs,
}

fn shaping(flags: &[&str]) -> Shaping {
    let typed = Typed::try_parse_from(std::iter::once("estimate").chain(flags.iter().copied()))
        .expect("the flags are the command line's own");
    Shaping::of(&typed.job).expect("the flags name no profile file")
}

#[test]
fn a_flag_over_the_autopilot_runs_its_step_always_with_the_value_typed() {
    let plan = built_in("prepare").expect("prepare is built in");
    let recipe = folded(Some(&plan), &shaping(&["--hollow", "3"]));
    let hollow = recipe
        .steps
        .iter()
        .find(|step| step.kind == StepKind::Hollow)
        .expect("prepare hollows");
    assert_eq!(
        hollow.when, None,
        "--hollow runs the step whatever its when"
    );
    assert_eq!(hollow.set.get("wall_mm"), Some(&Setting::Number(3.0)));
    let untouched = recipe
        .steps
        .iter()
        .find(|step| step.kind == StepKind::Orient);
    assert!(
        untouched.is_some_and(|step| step.when.is_some()),
        "a step no flag shapes keeps its condition"
    );
    validate(recipe).expect("the folded autopilot still runs");
}

#[test]
fn flags_with_no_recipe_become_its_steps_in_the_command_line_s_order() {
    let flags = ["--hollow", "2", "--drain-at", "auto", "--supports", "auto"];
    let recipe = folded(None, &shaping(&flags));
    let kinds: Vec<StepKind> = recipe.steps.iter().map(|step| step.kind).collect();
    assert_eq!(
        kinds,
        [StepKind::Hollow, StepKind::Drain, StepKind::Supports]
    );
    assert_eq!(
        recipe.steps[1].set.get("at"),
        Some(&Setting::Word("auto".to_owned()))
    );
    validate(recipe).expect("a recipe of flags runs");
}

#[test]
fn no_flags_and_no_recipe_is_a_recipe_of_no_steps() {
    assert!(folded(None, &shaping(&[])).steps.is_empty());
}
