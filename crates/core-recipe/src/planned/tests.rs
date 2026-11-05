use core_geometry::{Transform, Vec3, cuboid, diagnose};

use super::*;
use crate::condition::Value;
use crate::recipe::Recipe;
use crate::validate::validate;

fn plan(body: &str) -> Plan {
    let recipe = Recipe::from_toml(&format!("name = \"t\"\n{body}")).expect("the fixture reads");
    validate(recipe).expect("the fixture is a valid recipe")
}

/// `saves_ml` of a 40 mm cube measured as `plan` stands to it.
fn saves_ml(plan: &Plan, wall_mm: Option<Scalar>) -> Scalar {
    let cube = cuboid(Vec3::ZERO, Vec3::splat(40.0));
    let diagnostics = diagnose(&cube);
    let model = Model {
        mesh: &cube,
        transform: &Transform::default(),
        diagnostics: &diagnostics,
        turned: false,
        hollow: false,
        fits: true,
    };
    match measure_planned(plan, &model, wall_mm, &Metrics::default()).get(Metric::SavesMl) {
        Some(Value::Number(ml)) => ml,
        other => panic!("saves_ml is a number, got {other:?}"),
    }
}

#[test]
fn the_first_hollow_steps_wall_is_planned_outright_or_worked_out_at_its_index() {
    let outright = plan("[[step]]\ndo = \"hollow\"\nset = { wall_mm = 3 }\n");
    assert_eq!(planned_wall(&outright), (3.0, None));
    let worked = plan(
        "[[step]]\ndo = \"check\"\n[[step]]\ndo = \"hollow\"\nset = { wall_mm = \"= height_mm / 20\" }\n",
    );
    assert_eq!(
        planned_wall(&worked),
        (2.0, Some(1)),
        "Hollow's default until reached"
    );
    let none = plan("[[step]]\ndo = \"check\"\n");
    assert_eq!(planned_wall(&none), (2.0, None));
}

#[test]
fn saves_ml_is_reckoned_at_the_wall_the_step_works_out() {
    // 40³ less 6 · 40² surface times the wall, in ml.
    let worked = plan("[[step]]\ndo = \"hollow\"\nset = { wall_mm = \"= height_mm / 10\" }\n");
    let at_4 = (64_000.0 - 9_600.0 * 4.0) / 1000.0;
    let found = saves_ml(&worked, None);
    assert!(
        (found - at_4).abs() < 1e-3,
        "4 mm wall saves {at_4} ml, found {found}"
    );
    let fixed = saves_ml(&worked, Some(1.0));
    let at_1 = (64_000.0 - 9_600.0) / 1000.0;
    assert!(
        (fixed - at_1).abs() < 1e-3,
        "a fixed 1 mm wall saves {at_1} ml, found {fixed}"
    );
}
