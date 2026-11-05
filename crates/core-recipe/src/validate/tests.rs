use super::*;
use crate::builtin::built_in;
use crate::condition::Kind;

fn plan(body: &str) -> Result<Plan, RecipeError> {
    validate(Recipe::from_toml(&format!("name = \"t\"\n{body}")).expect("the fixture reads"))
}

fn step(kind: &str, extra: &str) -> String {
    format!("[[step]]\ndo = \"{kind}\"\n{extra}\n")
}

#[test]
fn built_in_prepare_validates() {
    built_in("prepare").expect("prepare is a valid recipe");
}

#[test]
fn drain_before_hollow_is_accepted_without_a_note() {
    let plan = plan(&(step("drain", "") + &step("hollow", ""))).expect("any order is allowed");
    assert!(plan.notes().is_empty());
}

#[test]
fn orient_after_supports_is_noted() {
    let plan = plan(&(step("supports", "") + &step("orient", ""))).expect("it is only noted");
    assert_eq!(
        plan.notes(),
        [Note {
            step: 1,
            kind: StepKind::Orient,
            over: 0,
            over_kind: StepKind::Supports
        }]
    );
}

#[test]
fn orient_over_a_drain_is_noted_only_for_auto_holes() {
    let auto = plan(&(step("drain", "") + &step("orient", ""))).expect("valid");
    assert_eq!(auto.notes().len(), 1);
    let points =
        plan(&(step("drain", "set = { at = \"points\" }") + &step("orient", ""))).expect("valid");
    assert!(points.notes().is_empty());
}

#[test]
fn every_row_of_the_order_table_is_noted() {
    let rows = [
        ("supports", "hollow"),
        ("hollow", "repair"),
        ("drain", "repair"),
        ("supports", "supports"),
    ];
    for (earlier, later) in rows {
        let plan = plan(&(step(earlier, "") + &step(later, ""))).expect("valid");
        assert_eq!(plan.notes().len(), 1, "{later} over {earlier}");
    }
}

#[test]
fn disabled_steps_leave_no_notes() {
    let plan = plan(&(step("supports", "enabled = false") + &step("orient", ""))).expect("valid");
    assert!(plan.notes().is_empty());
}

#[test]
fn trapped_needs_a_drain_or_check_before() {
    let hollow = step("hollow", "when = \"trapped > 0\"");
    assert!(matches!(
        plan(&hollow),
        Err(RecipeError::NotYetKnown { step: 0, name, after: StepKind::Drain }) if name == "trapped"
    ));
    plan(&(step("drain", "") + &hollow)).expect("a drain measured it");
}

#[test]
fn disabled_drain_does_not_measure() {
    let text = step("drain", "enabled = false") + &step("hollow", "when = \"trapped > 0\"");
    assert!(matches!(
        plan(&text),
        Err(RecipeError::NotYetKnown { step: 1, .. })
    ));
}

#[test]
fn check_retry_may_read_what_the_check_measures() {
    plan(&step(
        "check",
        "retry = { while = \"islands > 0\", max = 2 }",
    ))
    .expect("check measures");
    let later = step("hollow", "when = \"islands > 0\"");
    assert!(matches!(plan(&later), Err(RecipeError::NotYetKnown { .. })));
}

#[test]
fn retry_past_the_limit_is_refused() {
    let text = step("check", "retry = { while = \"islands > 0\", max = 6 }");
    assert!(matches!(
        plan(&text),
        Err(RecipeError::RetryTooMany {
            step: 0,
            max: 6,
            limit: crate::recipe::MAX_RETRY
        })
    ));
}

#[test]
fn unknown_name_is_refused() {
    let text = step("hollow", "when = \"weight > 1\"");
    assert!(matches!(plan(&text), Err(RecipeError::UnknownName { name }) if name == "weight"));
}

#[test]
fn recipe_var_is_a_number_in_conditions() {
    let text = "[vars]\nbig = 3\n".to_owned() + &step("hollow", "when = \"height_mm > big\"");
    plan(&text).expect("a var is a number");
}

#[test]
fn var_named_like_a_metric_shadows_it() {
    let text = "[vars]\nfaces = 3\n".to_owned() + &step("hollow", "");
    assert!(matches!(plan(&text), Err(RecipeError::Shadowed { name }) if name == "faces"));
}

#[test]
fn condition_of_the_wrong_kind_is_refused() {
    let text = step("hollow", "when = \"height_mm\"");
    assert!(matches!(
        plan(&text),
        Err(RecipeError::WrongKind {
            expected: Kind::Bool,
            found: Kind::Number,
            ..
        })
    ));
    let text = step("hollow", "set = { wall_mm = \"= sound\" }");
    assert!(matches!(
        plan(&text),
        Err(RecipeError::WrongKind {
            expected: Kind::Number,
            found: Kind::Bool,
            ..
        })
    ));
    let text = step("hollow", "set = { infill = \"= faces\" }");
    assert!(matches!(
        plan(&text),
        Err(RecipeError::WrongKind {
            expected: Kind::Bool,
            ..
        })
    ));
}

#[test]
fn expression_for_a_word_setting_is_refused() {
    let text = step("repair", "set = { method = \"= faces\" }");
    assert!(matches!(plan(&text), Err(RecipeError::BadSetting { .. })));
}
