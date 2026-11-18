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

#[test]
fn edits_on_a_model_rather_than_a_project_are_refused() {
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
        stderr(&output).contains("take a project"),
        "{}",
        stderr(&output)
    );
}
