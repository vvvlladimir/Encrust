//! `--recipe`: a model taken step by step through a recipe file or a built-in one, each step
//! decided on the model as the steps before left it (ADR 0403).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

use serde_json::Value;

use crate::support::*;

/// `text` written as the recipe file `name`.toml.
fn recipe_file(name: &str, text: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("encrust-cli-tests");
    fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join(format!("{name}.toml"));
    fs::write(&path, text).expect("write the recipe");
    path
}

/// A 16 mm cube on the tests' panel, run by `command` through `recipe` with `extra`.
fn run(command: &str, name: &str, recipe: &Path, extra: &[&str]) -> Output {
    let model = write_box_stl(name, 16.0, 12, 0);
    let panel = test_panel();
    let mut args = vec![
        command,
        model.to_str().unwrap(),
        "--profile",
        panel.to_str().unwrap(),
        "--layer-height",
        "0.5",
        "--recipe",
        recipe.to_str().unwrap(),
        "--json",
    ];
    args.extend_from_slice(extra);
    encrust(&args)
}

/// The same, sliced into a directory of its own, which must succeed.
fn sliced(name: &str, recipe: &str, extra: &[&str]) -> Value {
    let out = output_dir(name);
    let recipe = recipe_file(name, recipe);
    let mut args = vec!["-o", out.to_str().unwrap()];
    args.extend_from_slice(extra);
    let output = run("slice", name, &recipe, &args);
    assert!(
        output.status.success(),
        "the run failed:\n{}",
        stderr(&output)
    );
    json(&output)
}

/// The record of step `index` in a model's report.
fn step(report: &Value, index: usize) -> &Value {
    &report["recipe"]["steps"][index]
}

#[test]
fn points_drilled_before_the_hollow_are_a_recess_that_drains_nothing() {
    let report = sliced(
        "recipe-recess",
        r#"
        name = "Recess"
        [[step]]
        do = "drain"
        set = { at = "points", points = [[0.5, 0.5, 1.0]] }
        [[step]]
        do = "hollow"
        set = { wall_mm = 2 }
        "#,
        &[],
    );
    assert_eq!(report["recipe"]["name"], "Recess");
    assert_eq!(step(&report, 0)["decision"], "run", "{report}");
    assert_eq!(
        step(&report, 0)["holes"],
        1,
        "one hole on the top: {report}"
    );
    assert_eq!(report["hollow"]["wall_mm"], 2.0);
    assert!(
        report["slicing"]["trapped_pockets"].as_u64() >= Some(1),
        "a walled recess does not reach the cavity: {report}"
    );
}

#[test]
fn points_set_into_a_shell_stop_at_their_depth_and_leave_the_cavity_shut() {
    let report = sliced(
        "recipe-shallow-point",
        r#"
        name = "Shallow"
        [[step]]
        do = "hollow"
        set = { wall_mm = 2 }
        [[step]]
        do = "drain"
        set = { at = "points", points = [[0.5, 0.5, 1.0]], depth_mm = 0.5 }
        "#,
        &[],
    );
    assert_eq!(
        step(&report, 1)["holes"],
        1,
        "one hole on the top: {report}"
    );
    assert!(
        report["slicing"]["trapped_pockets"].as_u64() >= Some(1),
        "0.5 mm stops inside the 2 mm wall: {report}"
    );
}

#[test]
fn a_gate_on_resin_left_trapped_stops_the_run_and_writes_nothing() {
    let name = "recipe-gate";
    let out = output_dir(name);
    let recipe = recipe_file(
        name,
        r#"
        name = "Gated"
        [[step]]
        do = "hollow"
        [[step]]
        do = "check"
        [[step]]
        do = "gate"
        stop_if = "trapped > 0"
        message = "Resin stays trapped; drill it by hand"
        "#,
    );
    let output = run("slice", name, &recipe, &["-o", out.to_str().unwrap()]);
    assert!(
        !output.status.success(),
        "the gate holds: {}",
        stdout(&output)
    );
    assert!(
        stderr(&output).contains("Resin stays trapped; drill it by hand"),
        "{}",
        stderr(&output)
    );
    assert!(!out.exists(), "nothing was written");
}

#[test]
fn a_step_that_fails_with_on_fail_skip_is_noted_and_the_run_goes_on() {
    let report = sliced(
        "recipe-skip",
        r#"
        name = "Skipping"
        [[step]]
        do = "hollow"
        [[step]]
        do = "drain"
        on_fail = "skip"
        set = { diameter_mm = 0 }
        [[step]]
        do = "drain"
        "#,
        &[],
    );
    let failed = step(&report, 1)["note"].as_str().unwrap_or_default();
    assert!(failed.contains("no width"), "{report}");
    assert!(step(&report, 2)["holes"].as_u64() > Some(0), "{report}");
    assert_eq!(
        report["slicing"]["trapped_pockets"], 0,
        "the next drain ran: {report}"
    );
}

#[test]
fn a_step_that_fails_stops_the_run_naming_it() {
    let name = "recipe-stop";
    let recipe = recipe_file(
        name,
        "name = \"Stopping\"\n[[step]]\ndo = \"hollow\"\nset = { keep_solid_under_mm = -1 }\n",
    );
    let output = run("estimate", name, &recipe, &[]);
    assert!(!output.status.success());
    let said = stderr(&output);
    assert!(
        said.contains("step 0 (hollow)") && said.contains("kept solid under a positive thickness"),
        "{said}"
    );
}

#[test]
fn a_check_supports_its_islands_again_at_most_max_times() {
    let report = sliced(
        "recipe-retry",
        r#"
        name = "Retrying"
        [[step]]
        do = "check"
        retry = { while = "islands >= 0", max = 2 }
        "#,
        &[],
    );
    assert_eq!(
        step(&report, 0)["retries"],
        2,
        "the retry always holds: {report}"
    );
    assert!(report["recipe"]["steps"][0]["read"].is_object());
}

#[test]
fn a_save_step_writes_the_templated_name_into_its_folder() {
    let name = "recipe-save";
    let folder = output_dir(name);
    let recipe = recipe_file(
        name,
        &format!(
            "name = \"Saving\"\n[[step]]\ndo = \"save\"\nset = {{ folder = {:?}, name = \"{{model}}-saved\" }}\n",
            folder.display().to_string()
        ),
    );
    let output = run("slice", name, &recipe, &[]);
    assert!(output.status.success(), "{}", stderr(&output));
    let saved: Vec<String> = fs::read_dir(&folder)
        .expect("the folder was made")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        saved
            .iter()
            .any(|file| file.starts_with(&format!("{name}-saved."))),
        "the model's stem and the template's tail: {saved:?}"
    );
    let report = json(&output);
    let written = report["output"].as_str().unwrap_or_default();
    assert!(
        written.contains("-saved."),
        "the report is of the saved file: {report}"
    );
}

#[test]
fn a_recipe_that_saves_nothing_needs_an_output() {
    let name = "recipe-nowhere";
    let recipe = recipe_file(name, "name = \"Nowhere\"\n[[step]]\ndo = \"hollow\"\n");
    let output = run("slice", name, &recipe, &[]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("nothing would be written"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn recipe_prepare_reports_what_prepare_does() {
    let model = write_box_stl("recipe-prepare", 30.0, 12, 0);
    let panel = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/project-panel.toml");
    let prepared = |asked: &[&str]| {
        let mut args = vec![
            "estimate",
            model.to_str().unwrap(),
            "--profile",
            panel.to_str().unwrap(),
            "--layer-height",
            "0.5",
            "--center",
            "--json",
        ];
        args.extend_from_slice(asked);
        let output = encrust(&args);
        assert!(output.status.success(), "{}", stderr(&output));
        json(&output)["models"][0]["prepared"].clone()
    };
    let by_flag = prepared(&["--prepare"]);
    assert_eq!(by_flag["oriented"], true, "{by_flag}");
    assert_eq!(by_flag, prepared(&["--recipe", "prepare"]));
}

#[test]
fn a_recipe_file_that_does_not_read_fails_naming_why() {
    let name = "recipe-invalid";
    let recipe = recipe_file(name, "name = \"Broken\"\n[[step]]\ndo = \"polish\"\n");
    let output = run("estimate", name, &recipe, &[]);
    assert!(!output.status.success());
    let said = stderr(&output);
    assert!(
        said.contains("is not a recipe") && said.contains("there is no step `polish`"),
        "{said}"
    );
}

#[test]
fn a_typed_hollow_wins_over_the_recipes_wall() {
    let report = sliced(
        "recipe-typed",
        "name = \"Walled\"\n[[step]]\ndo = \"hollow\"\nset = { wall_mm = 2 }\n",
        &["--hollow", "3"],
    );
    assert_eq!(report["hollow"]["wall_mm"], 3.0, "{report}");
    assert_eq!(step(&report, 0)["settings"]["wall_mm"], 3.0, "{report}");
}

#[test]
fn a_typed_hollow_hollows_where_the_recipes_condition_would_not() {
    let report = sliced(
        "recipe-forced",
        "name = \"Never\"\n[[step]]\ndo = \"hollow\"\nwhen = \"false\"\n",
        &["--hollow", "3"],
    );
    assert_eq!(step(&report, 0)["decision"], "run", "{report}");
    assert_eq!(report["hollow"]["wall_mm"], 3.0, "{report}");
}

#[test]
fn a_typed_flag_with_no_step_of_its_kind_is_refused_naming_it() {
    let name = "recipe-unstepped";
    let recipe = recipe_file(name, "name = \"Checks\"\n[[step]]\ndo = \"check\"\n");
    let output = run("estimate", name, &recipe, &["--supports", "auto"]);
    assert!(!output.status.success(), "{}", stdout(&output));
    let said = stderr(&output);
    assert!(
        said.contains("--supports") && said.contains("no supports step"),
        "{said}"
    );
}

#[test]
fn a_typed_orient_turns_the_model_once_at_its_step() {
    let report = sliced(
        "recipe-orient",
        "name = \"Turned\"\n[[step]]\ndo = \"orient\"\nwhen = \"!turned\"\n",
        &["--orient"],
    );
    assert_eq!(
        step(&report, 0)["decision"],
        "run",
        "the step runs, not the import: {report}"
    );
}

#[test]
fn a_hollow_step_keeps_a_model_thinner_than_keep_solid_under_mm_solid() {
    let hollowed = |name: &str, kept: &str| {
        let recipe =
            format!("name = \"Kept\"\n[[step]]\ndo = \"hollow\"\nset = {{ wall_mm = 2{kept} }}\n");
        sliced(name, &recipe, &[])["hollow"]["cavity_mm3"]
            .as_f64()
            .expect("a cavity")
    };
    let open = hollowed("recipe-kept-open", "");
    let kept = hollowed("recipe-kept-solid", ", keep_solid_under_mm = 20");
    // The 16 mm cube's cavity is 12 mm across, 1728 mm3, and the whole cube is under 20 mm.
    assert!(
        (open - 1728.0).abs() < 1728.0 * 0.02,
        "1728 mm3 open, got {open}"
    );
    assert_eq!(kept, 0.0, "a 16 mm cube is under 20 mm, so 0 mm3 of cavity");
}

/// The first layer's lit pixels of an 8 mm cube, hollowed and drilled at the top's middle by
/// a drain step setting `plugs`: the tests' 20 mm panel has no room for plugs past the 16 mm one.
fn first_layer_plugged(name: &str, plugs: &str) -> usize {
    let recipe = recipe_file(
        name,
        &format!(
            "name = \"Plugged\"\n[[step]]\ndo = \"hollow\"\n\
             [[step]]\ndo = \"drain\"\nset = {{ at = \"points\", points = [[0.5, 0.5, 1.0]]{plugs} }}\n"
        ),
    );
    let model = write_box_stl(name, 8.0, 12, 0);
    let (panel, out) = (test_panel(), output_dir(name));
    let output = encrust(&[
        "slice",
        model.to_str().unwrap(),
        "--profile",
        panel.to_str().unwrap(),
        "--layer-height",
        "1",
        "--recipe",
        recipe.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
    ]);
    assert!(
        output.status.success(),
        "the run failed:\n{}",
        stderr(&output)
    );
    lit_pixels(&out, 0)
}

#[test]
fn a_drain_step_with_plugs_lays_a_plug_per_hole_and_without_none() {
    // The 8 mm cube's floor is 6 400 pixels; a 3 mm hole's plug, 1.35 mm in radius after
    // the 0.15 mm gap, adds about 570 beside it.
    let plugged = first_layer_plugged("recipe-plugs", ", plugs = true") as f32;
    assert!(
        (plugged - 6_970.0).abs() < 150.0,
        "the cube and one plug, about 6 970 pixels, got {plugged}"
    );
    for (name, plugs) in [
        ("recipe-no-plugs", ", plugs = false"),
        ("recipe-plugs-left-out", ""),
    ] {
        let bare = first_layer_plugged(name, plugs) as f32;
        assert!(
            (bare - 6_400.0).abs() < 150.0,
            "the cube alone, about 6 400 pixels, got {bare} with `{plugs}`"
        );
    }
}

/// A cup open at the top, 16 mm across with 2 mm walls, whose floor at 5 to 7 mm stands on
/// four legs clear of the plate.
fn cup_on_legs(name: &str) -> PathBuf {
    let leg = |x: f32, y: f32| ([x, y, 0.0], [x + 3.0, y + 3.0, 5.0]);
    write_boxes_stl(
        name,
        &[
            leg(0.0, 0.0),
            leg(13.0, 0.0),
            leg(0.0, 13.0),
            leg(13.0, 13.0),
            ([0.0, 0.0, 5.0], [16.0, 16.0, 7.0]),
            ([0.0, 0.0, 7.0], [16.0, 2.0, 15.0]),
            ([0.0, 14.0, 7.0], [16.0, 16.0, 15.0]),
            ([0.0, 2.0, 7.0], [2.0, 14.0, 15.0]),
            ([14.0, 2.0, 7.0], [16.0, 14.0, 15.0]),
        ],
    )
}

/// The report of `model` sliced through `recipe`, on the tests' panel at 0.25 mm layers.
fn cup_report(name: &str, model: &Path, recipe: &str) -> Value {
    let out = output_dir(name);
    let (recipe, panel) = (recipe_file(name, recipe), test_panel());
    let output = encrust(&[
        "slice",
        model.to_str().unwrap(),
        "--profile",
        panel.to_str().unwrap(),
        "--layer-height",
        "0.25",
        "--recipe",
        recipe.to_str().unwrap(),
        "--json",
        "-o",
        out.to_str().unwrap(),
    ]);
    assert!(
        output.status.success(),
        "the run failed:\n{}",
        stderr(&output)
    );
    json(&output)
}

#[test]
fn a_check_asked_to_drill_cups_leaves_none_to_report() {
    let model = cup_on_legs("recipe-cup");
    let plain = "name = \"Plain\"\n[[step]]\ndo = \"check\"\n";
    let before = cup_report("recipe-cup-plain", &model, plain);
    assert_eq!(before["cured"]["suction_cups"], 1, "the walls seal one cup");
    let drilling = "name = \"Drill\"\n[[step]]\ndo = \"check\"\nset = { drill_cups = true }\n";
    let after = cup_report("recipe-cup-drill", &model, drilling);
    assert_eq!(
        after["cured"]["suction_cups"], 0,
        "the hole lets the air in"
    );
}

/// A slab 16 mm long at 6 to 10 mm, held by one 1.5 mm column at its end.
fn slab_on_a_neck(name: &str) -> PathBuf {
    write_boxes_stl(
        name,
        &[
            ([0.0, 0.0, 0.0], [1.5, 1.5, 6.0]),
            ([0.0, 0.0, 6.0], [16.0, 6.0, 10.0]),
        ],
    )
}

#[test]
fn a_check_asked_to_hold_weak_necks_stands_a_support_under_the_piece() {
    let model = slab_on_a_neck("recipe-neck");
    let plain = "name = \"Plain\"\n[[step]]\ndo = \"check\"\n";
    let before = cup_report("recipe-neck-plain", &model, plain);
    let levers = before["cured"]["levers"]
        .as_u64()
        .expect("the report counts levers");
    assert!(
        levers > 0,
        "a 16 mm slab on a 1.5 mm column is a lever: {:#}",
        before["cured"]
    );
    let holding = "name = \"Hold\"\n[[step]]\ndo = \"check\"\nset = { hold_levers = true }\n";
    let after = cup_report("recipe-neck-hold", &model, holding);
    let left = after["cured"]["levers"]
        .as_u64()
        .expect("the report counts levers");
    assert!(
        left < levers,
        "a support under the slab takes the pull off the neck: {levers} before, {left} after"
    );
}
