//! `estimate --recipe-out`: the recipe a run takes, the flags typed written into its steps.

use crate::support::*;

#[test]
fn the_autopilot_written_out_hollows_at_the_wall_typed_whatever_its_condition() {
    let model = write_box_stl("recipe-out-cube", 16.0, 12, 0);
    let panel = test_panel();
    let out = output_file("recipe-out.toml");
    let output = estimate(&[
        model.to_str().unwrap(),
        "--profile",
        panel.to_str().unwrap(),
        "--layer-height",
        "0.5",
        "--prepare",
        "--hollow",
        "2.5",
        "--recipe-out",
        out.to_str().unwrap(),
    ]);
    assert!(
        output.status.success(),
        "the run failed:\n{}",
        stderr(&output)
    );
    let text = std::fs::read_to_string(&out).expect("--recipe-out wrote the file");
    let recipe = core_recipe::Recipe::from_toml(&text).expect("the file is a recipe");
    let hollow = recipe
        .steps
        .iter()
        .find(|step| step.kind == core_recipe::StepKind::Hollow)
        .expect("the autopilot hollows");
    assert_eq!(hollow.when, None);
    assert_eq!(
        hollow.set.get("wall_mm"),
        Some(&core_recipe::Setting::Number(2.5))
    );
}
