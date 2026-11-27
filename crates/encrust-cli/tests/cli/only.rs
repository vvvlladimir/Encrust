//! The objects of a plate by id: listed by `inspect`, shaped by `--only` or every one, on a
//! project as on models, and drawn alone by `render --only` (ADR 0458).

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::support::*;

fn machine() -> [String; 4] {
    let panel = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/project-panel.toml");
    [
        "--profile".into(),
        panel.display().to_string(),
        "--material".into(),
        test_resin().display().to_string(),
    ]
}

fn run(command: &str, args: &[&str]) -> std::process::Output {
    let machine = machine();
    let mut all = vec![command];
    all.extend_from_slice(args);
    all.extend(machine.iter().map(String::as_str));
    encrust(&all)
}

fn succeeded(output: &std::process::Output) -> Value {
    assert!(output.status.success(), "{}", stderr(output));
    json(output)
}

/// A 20 mm cube cut in two at 10 mm and spread apart, written as a project at `out`.
fn halves(name: &str, out: &Path) {
    let cube = write_box_stl(name, 20.0, 12, 0);
    let output = run(
        "estimate",
        &[
            cube.to_str().unwrap(),
            "--cut",
            "z:10",
            "--arrange",
            "--project-out",
            out.to_str().unwrap(),
            "--json",
        ],
    );
    succeeded(&output);
}

fn objects(project: &Path) -> Vec<Value> {
    let listed = succeeded(&encrust(&["inspect", project.to_str().unwrap(), "--json"]));
    listed["objects"]
        .as_array()
        .expect("inspect lists the objects")
        .clone()
}

#[test]
fn a_cut_model_is_hollowed_and_supported_half_by_half_in_one_run() {
    let cube = write_box_stl("only-cut-hollow", 20.0, 12, 0);
    let report = succeeded(&run(
        "estimate",
        &[
            cube.to_str().unwrap(),
            "--cut",
            "z:10",
            "--arrange",
            "--hollow",
            "2",
            "--supports",
            "medium",
            "--json",
        ],
    ));
    let models = report["models"].as_array().expect("a model a half");
    assert_eq!(models.len(), 2, "{report}");
    for half in models {
        assert!(
            half["id"].as_u64().is_some(),
            "each half names its id: {half}"
        );
        assert!(!half["hollow"].is_null(), "each half is hollowed: {half}");
        assert!(
            !half["supports"].is_null(),
            "each half stands on supports: {half}"
        );
    }
}

#[test]
fn only_the_object_named_is_hollowed_on_a_project() {
    let first = output_file("only-halves.encrust");
    halves("only-halves", &first);
    let ids: Vec<u64> = objects(&first)
        .iter()
        .filter_map(|object| object["id"].as_u64())
        .collect();
    assert_eq!(ids.len(), 2, "two halves listed: {ids:?}");

    let next = output_file("only-one-hollow.encrust");
    let named = ids[1].to_string();
    succeeded(&run(
        "estimate",
        &[
            first.to_str().unwrap(),
            "--only",
            &named,
            "--hollow",
            "2",
            "--project-out",
            next.to_str().unwrap(),
            "--json",
        ],
    ));
    let hollow: Vec<(u64, bool)> = (objects(&next).iter())
        .map(|object| (object["id"].as_u64().unwrap_or(0), object["hollow"] == true))
        .collect();
    assert_eq!(
        hollow,
        [(ids[0], false), (ids[1], true)],
        "the one named alone"
    );
}

#[test]
fn an_id_no_object_has_is_refused() {
    let first = output_file("only-unknown.encrust");
    halves("only-unknown", &first);
    let output = run(
        "estimate",
        &[first.to_str().unwrap(), "--only", "99", "--hollow", "2"],
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("--only 99"), "{}", stderr(&output));
}

#[test]
fn a_point_on_one_model_needs_only_when_several_are_shaped() {
    let first = output_file("only-pointing.encrust");
    halves("only-pointing", &first);
    let output = run(
        "estimate",
        &[
            first.to_str().unwrap(),
            "--support-at",
            "5,5,0",
            "--supports",
            "medium",
        ],
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("--only"), "{}", stderr(&output));
}

#[test]
fn render_only_draws_the_object_named() {
    let first = output_file("only-render.encrust");
    halves("only-render", &first);
    let id = objects(&first)[0]["id"]
        .as_u64()
        .expect("an id")
        .to_string();
    let picture = output_file("only-render.png");
    let output = run(
        "render",
        &[
            first.to_str().unwrap(),
            "--only",
            &id,
            "--view",
            "top",
            "--size",
            "64",
            "-o",
            picture.to_str().unwrap(),
        ],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(picture.is_file());
}
