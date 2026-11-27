//! `--orient-choices`, `--orient-pick` and `--orient-keep-clear`: the search's best ways to
//! stand a model, one of them taken, and places kept off the supports (ADR 0460).

use std::path::PathBuf;

use serde_json::Value;

use crate::support::*;

fn panel() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/project-panel.toml")
}

/// A leaning box estimated under `--orient` with medium supports and `extra`, as JSON.
fn oriented(name: &str, extra: &[&str]) -> Value {
    let path = write_leaning_box_stl(name, 10.0, 8.0, 12, 0);
    let panel = panel();
    let mut args = vec![
        path.to_str().unwrap(),
        "--profile",
        panel.to_str().unwrap(),
        "--layer-height",
        "0.5",
        "--center",
        "--orient",
        "--supports",
        "medium",
        "--json",
    ];
    args.extend_from_slice(extra);
    let output = estimate(&args);
    assert!(output.status.success(), "{}", stderr(&output));
    json(&output)
}

fn turn_of(report: &Value) -> Value {
    report["models"][0]["oriented"]["rotate_deg"].clone()
}

#[test]
fn the_choices_are_numbered_from_one_with_what_each_printed() {
    let report = oriented("choices", &["--orient-choices", "3"]);
    let choices = report["models"][0]["oriented"]["choices"]
        .as_array()
        .expect("choices are reported");
    assert!(!choices.is_empty() && choices.len() <= 3, "{report}");
    assert_eq!(choices[0]["pick"], 1);
    assert_eq!(
        choices[0]["rotate_deg"],
        turn_of(&report),
        "the first choice is the turn taken"
    );
    assert!(choices[0]["printed"]["layers"].as_u64().unwrap_or(0) > 0);
}

#[test]
fn a_pick_stands_the_model_the_way_that_choice_does() {
    let listed = oriented("pick-list", &["--orient-choices", "4"]);
    let choices = listed["models"][0]["oriented"]["choices"]
        .as_array()
        .expect("choices are reported");
    assert!(
        choices.len() >= 2,
        "a leaning box has more than one way: {listed}"
    );
    let picked = oriented("pick-second", &["--orient-pick", "2"]);
    assert_eq!(turn_of(&picked), choices[1]["rotate_deg"]);
}

#[test]
fn a_pick_past_the_choices_is_refused() {
    // A cube stands only a few ways a print can tell apart.
    let path = write_box_stl("pick-past", 10.0, 12, 0);
    let panel = panel();
    let output = estimate(&[
        path.to_str().unwrap(),
        "--profile",
        panel.to_str().unwrap(),
        "--orient",
        "--supports",
        "medium",
        "--orient-pick",
        "64",
    ]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
}

#[test]
fn a_pick_without_orient_is_an_argument_error() {
    let path = write_box_stl("pick-alone", 10.0, 12, 0);
    let output = estimate(&[path.to_str().unwrap(), "--orient-pick", "2"]);
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
}

#[test]
fn a_place_kept_clear_is_counted_on_every_choice() {
    let report = oriented(
        "keep-clear",
        &[
            "--orient-choices",
            "4",
            "--orient-keep-clear",
            "40,40,0",
            "--orient-keep-clear-radius",
            "200",
        ],
    );
    let choices = report["models"][0]["oriented"]["choices"]
        .as_array()
        .expect("choices are reported");
    for choice in choices {
        let printed = &choice["printed"];
        assert_eq!(
            printed["kept_clear_touched"], printed["contacts"],
            "a 200 mm reach touches every contact: {choice}"
        );
    }
}

#[test]
fn an_orient_with_no_printer_and_no_supports_is_still_proved_on_the_standard_printer() {
    let path = write_leaning_box_stl("standard", 10.0, 8.0, 12, 0);
    let output = estimate(&[
        path.to_str().unwrap(),
        "--orient",
        "--orient-choices",
        "2",
        "--json",
    ]);
    assert!(output.status.success(), "{}", stderr(&output));

    let report = json(&output);

    let choices = report["models"][0]["oriented"]["choices"]
        .as_array()
        .expect("choices are reported");
    assert!(
        choices[0]["printed"]["layers"].as_u64().unwrap_or(0) > 0,
        "the first way was printed in a trial: {report}"
    );
}

#[test]
fn inspect_stands_a_model_the_way_its_orientation_search_does() {
    let bar = core_geometry::cuboid(
        core_geometry::Vec3::ZERO,
        core_geometry::Vec3::new(10.0, 20.0, 30.0),
    );
    let path = write_mesh_stl("inspect-orient", &bar);
    let output = encrust(&[
        "--json",
        "inspect",
        path.to_str().unwrap(),
        "--orient",
        "--orient-goal",
        "shortest-print",
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    let height_mm = json(&output)["model"]["size_mm"][2]
        .as_f64()
        .expect("the size is reported");
    assert!(
        (height_mm - 10.0).abs() < 1e-3,
        "the shortest print lays the 30 mm bar on its widest face, 10 mm tall, got {height_mm}"
    );
}

#[test]
fn a_search_kept_by_one_run_answers_the_next_alike() {
    let kept = output_dir("kept-poses");
    let path = write_leaning_box_stl("kept-poses", 10.0, 8.0, 12, 0);
    let panel = panel();
    let args = [
        "estimate",
        path.to_str().unwrap(),
        "--profile",
        panel.to_str().unwrap(),
        "--layer-height",
        "0.5",
        "--center",
        "--orient",
        "--orient-choices",
        "3",
        "--json",
    ];
    let run = || {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_encrust"))
            .args(args)
            .env("ENCRUST_POSE_CACHE", &kept)
            .output()
            .expect("the encrust binary runs");
        assert!(output.status.success(), "{}", stderr(&output));
        json(&output)["models"][0]["oriented"]["choices"].clone()
    };
    let searched = run();
    let files = std::fs::read_dir(&kept).map_or(0, Iterator::count);
    assert_eq!(files, 1, "the search is kept in the folder named");
    assert_eq!(run(), searched, "the next run reads the same choices");
}
