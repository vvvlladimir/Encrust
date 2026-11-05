use super::*;
use crate::fixtures::{plan, step};

/// A 100 mm tall sound solid model, nothing measured after a step.
fn tall() -> Metrics {
    let mut metrics = Metrics::default();
    metrics.set(Metric::HeightMm, Value::Number(100.0));
    metrics.set(Metric::Defects, Value::Number(0.0));
    metrics.set(Metric::Sound, Value::Bool(true));
    metrics.set(Metric::Hollow, Value::Bool(false));
    metrics
}

fn reading(name: &str, value: Value) -> Reading {
    Reading {
        name: name.to_owned(),
        value,
    }
}

fn settings_of(decision: Decision) -> BTreeMap<String, Resolved> {
    match decision {
        Decision::Run { settings, .. } => settings,
        other => panic!("expected the step to run, got {other:?}"),
    }
}

#[test]
fn a_disabled_step_is_disabled_whatever_its_condition() {
    let plan = plan(&step("hollow", "enabled = false\nwhen = \"sound\""));
    assert_eq!(decide(&plan, 0, &tall()), Ok(Decision::Disabled));
}

#[test]
fn a_false_when_skips_with_what_it_read() {
    let plan = plan(&step("hollow", "when = \"!sound\""));
    let read = vec![reading("sound", Value::Bool(true))];
    assert_eq!(decide(&plan, 0, &tall()), Ok(Decision::Skip { read }));
}

#[test]
fn a_step_without_when_runs_with_its_settings() {
    let plan = plan(&step("hollow", "set = { wall_mm = 2.5, infill = true }"));
    let settings = settings_of(decide(&plan, 0, &tall()).expect("decided"));
    assert_eq!(settings["wall_mm"], Resolved::Number(2.5));
    assert_eq!(settings["infill"], Resolved::Bool(true));
}

#[test]
fn the_first_choice_that_holds_is_merged_over_set() {
    let choose = "set = { goal = \"auto\", max_tilt_deg = 30 }\nchoose = [\n\
        { when = \"height_mm > 50\", set = { goal = \"least-peel\" } },\n\
        { when = \"height_mm > 10\", set = { goal = \"shortest-print\" } },\n]";
    let plan = plan(&step("orient", choose));
    let settings = settings_of(decide(&plan, 0, &tall()).expect("decided"));
    assert_eq!(settings["goal"], Resolved::Word("least-peel".into()));
    assert_eq!(settings["max_tilt_deg"], Resolved::Number(30.0));
}

#[test]
fn an_expression_setting_is_worked_out_on_the_model() {
    let plan = plan(&step(
        "hollow",
        "set = { wall_mm = \"= clamp(height_mm / 25, 1.5, 3)\" }",
    ));
    let settings = settings_of(decide(&plan, 0, &tall()).expect("decided"));
    assert_eq!(
        settings["wall_mm"],
        Resolved::Number(3.0),
        "100 / 25 = 4 clamped to 3"
    );
}

#[test]
fn text_and_points_pass_through() {
    let plan = plan(
        &(step(
            "drain",
            "set = { at = \"points\", points = [[0.5, 0.5, 0.0]] }",
        ) + &step("supports", "set = { profile = \"light\" }")),
    );
    let drain = settings_of(decide(&plan, 0, &tall()).expect("decided"));
    assert_eq!(drain["at"], Resolved::Word("points".into()));
    assert_eq!(drain["points"], Resolved::Points(vec![[0.5, 0.5, 0.0]]));
    let supports = settings_of(decide(&plan, 1, &tall()).expect("decided"));
    assert_eq!(supports["profile"], Resolved::Text("light".into()));
}

#[test]
fn each_name_is_read_once_in_the_order_first_read() {
    let body = "[vars]\nbig = 30\n".to_owned()
        + &step(
            "hollow",
            "when = \"height_mm > big && sound && height_mm < 500\"",
        );
    let read = match decide(&plan(&body), 0, &tall()) {
        Ok(Decision::Run { read, .. }) => read,
        other => panic!("expected a run, got {other:?}"),
    };
    assert_eq!(
        read,
        [
            reading("height_mm", Value::Number(100.0)),
            reading("big", Value::Number(30.0)),
            reading("sound", Value::Bool(true)),
        ]
    );
}

#[test]
fn a_gate_whose_stop_if_holds_stops_with_its_message() {
    let plan = plan(&step(
        "gate",
        "stop_if = \"height_mm > 50\"\nmessage = \"too tall\"",
    ));
    assert_eq!(
        decide(&plan, 0, &tall()),
        Ok(Decision::Stop {
            message: "too tall".into(),
            read: vec![reading("height_mm", Value::Number(100.0))],
        })
    );
}

#[test]
fn a_gate_with_no_message_stops_with_an_empty_one() {
    let plan = plan(&step("gate", "stop_if = \"sound\""));
    assert!(matches!(
        decide(&plan, 0, &tall()),
        Ok(Decision::Stop { message, .. }) if message.is_empty()
    ));
}

#[test]
fn a_gate_whose_stop_if_fails_is_skipped() {
    let plan = plan(&step("gate", "stop_if = \"!sound\""));
    assert!(matches!(
        decide(&plan, 0, &tall()),
        Ok(Decision::Skip { .. })
    ));
}

#[test]
fn a_pause_says_its_message() {
    let plan = plan(&step("pause", "message = \"look at it\""));
    assert_eq!(
        decide(&plan, 0, &tall()),
        Ok(Decision::Pause {
            message: "look at it".into()
        })
    );
}

#[test]
fn a_step_past_the_end_is_no_such_step() {
    let plan = plan(&step("hollow", ""));
    assert_eq!(
        decide(&plan, 1, &tall()),
        Err(RecipeError::NoSuchStep { step: 1 })
    );
    assert_eq!(
        retry(&plan, 3, &tall(), 0),
        Err(RecipeError::NoSuchStep { step: 3 })
    );
}

#[test]
fn a_name_not_measured_yet_is_unmeasured() {
    let plan = plan(&(step("drain", "") + &step("gate", "stop_if = \"trapped > 0\"")));
    assert!(matches!(
        decide(&plan, 1, &tall()),
        Err(RecipeError::Unmeasured { name }) if name == "trapped"
    ));
}

#[test]
fn dividing_by_a_zero_metric_is_an_error_at_the_step() {
    let plan = plan(&step(
        "hollow",
        "set = { wall_mm = \"= height_mm / defects\" }",
    ));
    assert!(matches!(
        decide(&plan, 0, &tall()),
        Err(RecipeError::DivisionByZero { .. })
    ));
}

fn with_islands(islands: Scalar) -> Metrics {
    let mut metrics = tall();
    metrics.set(Metric::Islands, Value::Number(islands));
    metrics
}

#[test]
fn a_check_retries_while_its_condition_holds_up_to_max() {
    let plan = plan(&step(
        "check",
        "retry = { while = \"islands > 0\", max = 2 }",
    ));
    assert_eq!(retry(&plan, 0, &with_islands(3.0), 0), Ok(true));
    assert_eq!(retry(&plan, 0, &with_islands(3.0), 1), Ok(true));
    assert_eq!(
        retry(&plan, 0, &with_islands(3.0), 2),
        Ok(false),
        "max reached"
    );
    assert_eq!(retry(&plan, 0, &with_islands(0.0), 0), Ok(false));
}

#[test]
fn a_step_without_retry_never_retries() {
    let plan = plan(&step("check", ""));
    assert_eq!(retry(&plan, 0, &tall(), 0), Ok(false));
}

#[test]
fn preview_leaves_what_reads_an_unmeasured_name_until_reached() {
    let plan = plan(
        &(step("hollow", "when = \"sound\"")
            + &step("drain", "")
            + &step("gate", "stop_if = \"trapped > 0\"")),
    );
    let foreseen = preview(&plan, &tall());
    assert!(matches!(
        foreseen[0],
        Foreseen::Decided(Decision::Run { .. })
    ));
    assert!(matches!(
        foreseen[1],
        Foreseen::Decided(Decision::Run { .. })
    ));
    assert_eq!(foreseen[2], Foreseen::WhenReached);
}

#[test]
fn a_forced_step_runs_with_its_settings_whatever_its_when() {
    let unsound = plan(&step(
        "hollow",
        "when = \"!sound\"\nset = { wall_mm = 2.5 }",
    ));
    let settings = settings_of(decide_forced(&unsound, 0, &tall()).expect("decided"));
    assert_eq!(settings["wall_mm"], Resolved::Number(2.5));
    let off = plan(&step("hollow", "enabled = false"));
    assert_eq!(
        decide_forced(&off, 0, &tall()),
        Ok(Decision::Disabled),
        "a step kept out of the run stays out"
    );
}
