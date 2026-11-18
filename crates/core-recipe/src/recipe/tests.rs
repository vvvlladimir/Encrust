use super::*;
use crate::condition::Kind;

const EVERY_KIND: &str = r#"
name = "Everything"
format = 1

[vars]
big_ml = 30
slim = 0.5

[[step]]
do = "repair"
on_fail = "skip"
set = { method = "wrap" }

[[step]]
do = "orient"
when = "!turned"
set = { goal = "least-peel", max_tilt_deg = 30, clean_surface = true }
choose = [{ when = "height_mm > 120", set = { goal = "shortest-print" } }]

[[step]]
do = "hollow"
enabled = false
when = "saves_ml >= big_ml"
set = { wall_mm = "= clamp(height_mm / 50, 1.5, 3)", mode = "inside", infill = true, keep_solid_under_mm = 1 }

[[step]]
do = "drain"
set = { at = "points", points = [[0.5, 0.5, 0], [0.1, 0.2, 1]], diameter_mm = 2.5, depth_mm = 3, vent = "= hollow", plugs = false }

[[step]]
do = "supports"
set = { profile = "auto", cavity = "= hollow" }

[[step]]
do = "check"
retry = { while = "islands > 0", max = 3 }

[[step]]
do = "arrange"
set = { spacing_mm = 4 }

[[step]]
do = "gate"
stop_if = "trapped > 0"
message = "Resin stays trapped"

[[step]]
do = "pause"
message = "Look at it"

[[step]]
do = "save"
set = { folder = "out", name = "{model}", send = true }
"#;

fn error(text: &str) -> RecipeError {
    Recipe::from_toml(text).expect_err("the recipe is wrong")
}

#[test]
fn recipe_with_every_kind_and_setting_reads_back_equal() {
    let recipe = Recipe::from_toml(EVERY_KIND).expect("the fixture is a recipe");
    assert_eq!(recipe.steps.len(), StepKind::ALL.len());
    let again = Recipe::from_toml(&recipe.to_toml()).expect("to_toml writes a recipe");
    assert_eq!(again, recipe);
}

#[test]
fn a_number_is_written_as_typed_not_as_its_widening() {
    let recipe = Recipe::from_toml(
        "name = \"Thin\"\n[vars]\nslim = 0.8\n\
         [[step]]\ndo = \"hollow\"\nset = { wall_mm = 2.1, keep_solid_under_mm = 0.3 }\n",
    )
    .expect("the recipe reads");
    let text = recipe.to_toml();
    for written in ["slim = 0.8\n", "wall_mm = 2.1", "keep_solid_under_mm = 0.3"] {
        assert!(text.contains(written), "`{written}` in:\n{text}");
    }
    assert_eq!(Recipe::from_toml(&text).expect("reads back"), recipe);
}

#[test]
fn every_step_kind_is_found_by_its_name() {
    for kind in StepKind::ALL {
        assert_eq!(StepKind::from_name(kind.name()), Some(kind));
    }
}

#[test]
fn missing_format_is_format_one() {
    let recipe = Recipe::from_toml("name = \"x\"").expect("format defaults");
    assert!(recipe.steps.is_empty());
}

#[test]
fn newer_format_is_refused() {
    assert!(matches!(
        error("name = \"x\"\nformat = 2"),
        RecipeError::Format {
            found: 2,
            supported: FORMAT
        }
    ));
}

#[test]
fn text_that_is_not_toml_is_a_toml_error() {
    assert!(matches!(error("name = "), RecipeError::Toml { .. }));
    assert!(matches!(error("format = 1"), RecipeError::Toml { .. }));
}

#[test]
fn unknown_step_is_named() {
    let text = "name = \"x\"\n[[step]]\ndo = \"polish\"";
    assert!(matches!(error(text), RecipeError::UnknownStep { step: 0, name } if name == "polish"));
}

#[test]
fn setting_a_kind_does_not_take_is_unknown() {
    let text = "name = \"x\"\n[[step]]\ndo = \"repair\"\nset = { wall_mm = 2 }";
    assert!(matches!(
        error(text),
        RecipeError::UnknownSetting { step: 0, kind: StepKind::Repair, key } if key == "wall_mm"
    ));
    let text = "name = \"x\"\n[[step]]\ndo = \"repair\"\nmessage = \"hi\"";
    assert!(matches!(error(text), RecipeError::UnknownSetting { .. }));
}

#[test]
fn gate_without_stop_if_is_missing_it() {
    let text = "name = \"x\"\n[[step]]\ndo = \"gate\"";
    assert!(matches!(
        error(text),
        RecipeError::MissingSetting {
            step: 0,
            kind: StepKind::Gate,
            key: "stop_if"
        }
    ));
}

#[test]
fn check_retry_without_max_is_missing_it() {
    let text = "name = \"x\"\n[[step]]\ndo = \"check\"\nretry = { while = \"islands > 0\" }";
    assert!(matches!(
        error(text),
        RecipeError::MissingSetting { key: "max", .. }
    ));
}

#[test]
fn badly_typed_settings_are_refused_with_what_was_expected() {
    let cases = [
        (
            "repair",
            "{ method = \"melt\" }",
            "repair, close-holes, wrap, deep",
        ),
        ("hollow", "{ wall_mm = \"thick\" }", "a number"),
        ("hollow", "{ infill = 1 }", "true or false"),
        ("save", "{ folder = 3 }", "text"),
        (
            "drain",
            "{ points = [[0.5, 2, 0]] }",
            "a list of [x, y, z], each from 0 to 1",
        ),
        (
            "drain",
            "{ points = [[0.5, 0.5]] }",
            "a list of [x, y, z], each from 0 to 1",
        ),
    ];
    for (step, set, want) in cases {
        let text = format!("name = \"x\"\n[[step]]\ndo = \"{step}\"\nset = {set}");
        assert!(
            matches!(error(&text), RecipeError::BadSetting { expected, .. } if expected == want),
            "{set} should expect {want}"
        );
    }
    let text = "name = \"x\"\n[[step]]\ndo = \"repair\"\non_fail = \"explode\"";
    assert!(matches!(error(text), RecipeError::BadSetting { key, .. } if key == "on_fail"));
}

#[test]
fn expression_with_a_syntax_error_propagates() {
    let text = "name = \"x\"\n[[step]]\ndo = \"hollow\"\nset = { wall_mm = \"= 1 +\" }";
    assert!(matches!(error(text), RecipeError::Syntax { .. }));
    let text = "name = \"x\"\n[[step]]\ndo = \"hollow\"\nwhen = \"sound &&\"";
    assert!(matches!(error(text), RecipeError::Syntax { .. }));
}

#[test]
fn expression_setting_is_kept_as_an_expression() {
    let text = "name = \"x\"\n[[step]]\ndo = \"supports\"\nset = { cavity = \"= hollow\" }";
    let recipe = Recipe::from_toml(text).expect("an expression is a setting");
    let Some(Setting::Expr(condition)) = recipe.steps[0].set.get("cavity") else {
        panic!("cavity is not an expression");
    };
    assert_eq!(condition.kind(&|_| Some(Kind::Bool)), Ok(Kind::Bool));
}

#[test]
fn a_steps_conditions_are_its_when_gate_retry_rules_and_expressions() {
    let recipe = Recipe::from_toml(EVERY_KIND).expect("the fixture is a recipe");
    let texts = |index: usize| -> Vec<&str> {
        let step: &Step = &recipe.steps[index];
        step.conditions().into_iter().map(Condition::text).collect()
    };
    assert_eq!(texts(1), ["!turned", "height_mm > 120"]);
    assert_eq!(
        texts(2),
        ["saves_ml >= big_ml", "clamp(height_mm / 50, 1.5, 3)"]
    );
    assert_eq!(texts(5), ["islands > 0"]);
    assert_eq!(texts(7), ["trapped > 0"]);
    assert!(texts(8).is_empty(), "a pause holds none");
}
