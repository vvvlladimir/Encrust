//! `--edits` on a project, and `--project-out` writing the plate they leave (ADR 0466).

use std::fs;
use std::path::{Path, PathBuf};

use core_engine::project::{ObjectId, load};

use crate::support::*;

/// The window's project of one 20 mm cube, standing from z 5 to 25, object 1 once read.
fn project_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cube.encrust")
}

fn edits_file(name: &str, json: &str) -> PathBuf {
    let path = output_file(name);
    fs::write(&path, json).expect("the output folder takes a file");
    path
}

fn estimate_edited(edits: &Path, out: &Path) -> std::process::Output {
    let panel = project_fixture().with_file_name("project-panel.toml");
    let resin = test_resin();
    estimate(&[
        project_fixture().to_str().expect("a UTF-8 path"),
        "--profile",
        panel.to_str().expect("a UTF-8 path"),
        "--material",
        resin.to_str().expect("a UTF-8 path"),
        "--edits",
        edits.to_str().expect("a UTF-8 path"),
        "--project-out",
        out.to_str().expect("a UTF-8 path"),
        "--json",
    ])
}

#[test]
fn a_cut_edit_leaves_two_halves_in_the_project_written() {
    let edits = edits_file(
        "cut-edits.json",
        r#"[{"edit": "cut", "object": 1, "axis": "z", "at_mm": 15.0}]"#,
    );
    let out = output_file("cut-edited.encrust");

    let output = estimate_edited(&edits, &out);

    assert!(output.status.success(), "{}", stderr(&output));
    let project = load(&out).expect("the project written reads back");
    let ids: Vec<ObjectId> = (project.manifest.objects.iter())
        .map(|object| object.id)
        .collect();
    assert_eq!(
        ids,
        [ObjectId(2), ObjectId(3)],
        "the cube's two halves, given new ids"
    );
}

#[test]
fn an_edit_naming_an_object_not_on_the_plate_is_refused() {
    let edits = edits_file(
        "missing-object.json",
        r#"[{"edit": "cut", "object": 7, "axis": "z", "at_mm": 15.0}]"#,
    );
    let out = output_file("missing-object.encrust");

    let output = estimate_edited(&edits, &out);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("edit 1 of --edits"),
        "{}",
        stderr(&output)
    );
    assert!(
        !out.exists(),
        "nothing is written for a plate an edit refused"
    );
}

fn estimate_model(model: &Path, out: &Path, extra: &[&str]) -> std::process::Output {
    let panel = test_panel();
    let resin = test_resin();
    let mut args = vec![
        model.to_str().expect("a UTF-8 path"),
        "--profile",
        panel.to_str().expect("a UTF-8 path"),
        "--material",
        resin.to_str().expect("a UTF-8 path"),
        "--project-out",
        out.to_str().expect("a UTF-8 path"),
    ];
    args.extend_from_slice(extra);
    estimate(&args)
}

#[test]
fn a_model_is_written_as_a_project_of_one_object_as_imported() {
    let model = write_box_stl("model-to-project", 20.0, 12, 0);
    let out = output_file("model-to-project.encrust");

    let output = estimate_model(&model, &out, &["--center"]);

    assert!(output.status.success(), "{}", stderr(&output));
    let project = load(&out).expect("the project written reads back");
    assert_eq!(project.manifest.objects.len(), 1);
    assert_eq!(project.manifest.objects[0].id, ObjectId(1));
    let volume = core_geometry::signed_volume(&project.models[0].source);
    assert!(
        (volume - 8000.0).abs() < 0.01,
        "the 20 mm cube is {volume} mm3, expected 8000"
    );
}

#[test]
fn a_cut_edit_on_a_model_writes_its_two_halves() {
    let model = write_box_stl("model-cut", 20.0, 12, 0);
    let edits = edits_file(
        "model-cut.json",
        r#"[{"edit": "cut", "object": 1, "axis": "z", "at_mm": 10.0}]"#,
    );
    let out = output_file("model-cut.encrust");

    let output = estimate_model(
        &model,
        &out,
        &["--edits", edits.to_str().expect("a UTF-8 path")],
    );

    assert!(output.status.success(), "{}", stderr(&output));
    let project = load(&out).expect("the project written reads back");
    assert_eq!(project.manifest.objects.len(), 2, "the cube's two halves");
}

#[test]
fn hollowing_a_model_written_as_a_project_is_refused() {
    let model = write_box_stl("model-hollow", 20.0, 12, 0);
    let out = output_file("model-hollow.encrust");

    let output = estimate_model(&model, &out, &["--hollow", "2"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("--project-out writes models as imported"),
        "{}",
        stderr(&output)
    );
    assert!(!out.exists(), "nothing is written for a refused run");
}

#[test]
fn edits_on_a_model_without_project_out_are_refused() {
    let model = write_box_stl("edits-on-a-model", 10.0, 12, 0);
    let edits = edits_file("edits-on-a-model.json", "[]");

    let output = estimate(&[
        model.to_str().expect("a UTF-8 path"),
        "--profile",
        test_panel().to_str().expect("a UTF-8 path"),
        "--material",
        test_resin().to_str().expect("a UTF-8 path"),
        "--edits",
        edits.to_str().expect("a UTF-8 path"),
    ]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("needs --project-out"),
        "{}",
        stderr(&output)
    );
}
