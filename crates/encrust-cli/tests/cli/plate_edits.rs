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

/// The shell and the cavity a run writes into the project at `out`, or why it failed.
fn shell_written(out: &Path) -> (Vec<core_geometry::Vec3>, f32) {
    let project = load(out).expect("the project written reads back");
    let shell = (project.models[0].shell.as_ref()).expect("the cube is hollowed");
    let built = (project.manifest.objects[0].hollow.built.as_ref()).expect("its cavity is kept");
    (shell.vertices.clone(), built.cavity_mm3)
}

#[test]
fn a_hollow_by_its_flag_and_by_the_edit_write_the_same_project() {
    let model = write_box_stl("model-hollow", 20.0, 12, 0);
    let (by_flag, by_edit) = (
        output_file("model-hollow-flag.encrust"),
        output_file("model-hollow-edit.encrust"),
    );
    let edits = edits_file(
        "model-hollow.json",
        r#"[{"edit": "hollow", "object": 1, "settings": {"thickness_mm": 2.0,
            "mode": "internal", "precision": 0.5, "infill_on": false,
            "infill": {"kind": "hive", "cell_mm": 6.0, "density": 0.3, "wall_mm": null}}}]"#,
    );

    let flagged = estimate_model(&model, &by_flag, &["--hollow", "2", "--precision", "0.5"]);
    let edited = estimate_model(
        &model,
        &by_edit,
        &["--edits", edits.to_str().expect("a UTF-8 path")],
    );

    assert!(flagged.status.success(), "{}", stderr(&flagged));
    assert!(edited.status.success(), "{}", stderr(&edited));
    let (shell, cavity_mm3) = shell_written(&by_flag);
    assert_eq!(
        (shell, cavity_mm3),
        shell_written(&by_edit),
        "one hollow, two ways asked"
    );
    let side = cavity_side_mm(20.0, 2.0);
    assert!(
        (cavity_mm3 - side.powi(3)).abs() < side.powi(3) * 0.02,
        "a cavity of {side} mm a side, {} mm3, got {cavity_mm3}",
        side.powi(3)
    );
}

#[test]
fn edits_on_a_model_with_a_recipe_are_refused() {
    let model = write_box_stl("edits-with-a-recipe", 10.0, 12, 0);
    let out = output_file("edits-with-a-recipe.encrust");
    let edits = edits_file("edits-with-a-recipe.json", "[]");

    let edits_arg = edits.to_str().expect("a UTF-8 path");
    let output = estimate_model(&model, &out, &["--prepare", "--edits", edits_arg]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("a recipe prepares its own"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn a_model_prepared_by_a_recipe_is_written_as_the_plate_it_leaves() {
    let model = write_leaning_box_stl("project-prepared", 10.0, 8.0, 12, 0);
    let out = output_file("project-prepared.encrust");

    let panel = project_fixture().with_file_name("project-panel.toml");
    let output = estimate(&[
        model.to_str().expect("a UTF-8 path"),
        "--profile",
        panel.to_str().expect("a UTF-8 path"),
        "--material",
        test_resin().to_str().expect("a UTF-8 path"),
        "--center",
        "--prepare",
        "--project-out",
        out.to_str().expect("a UTF-8 path"),
    ]);

    assert!(output.status.success(), "{}", stderr(&output));
    let project = load(&out).expect("the prepared plate is written");
    assert_eq!(project.manifest.objects.len(), 1, "one model, prepared");
    let supports = &project.manifest.objects[0].supports;
    assert!(
        !supports.grown.is_empty() || !supports.frozen.is_empty(),
        "a leaning box is prepared on supports, which the plate keeps"
    );
}

fn sliced_plain(model: &Path, name: &str, extra: &[&str]) -> PathBuf {
    let out = output_dir(name);
    let (panel, resin) = (test_panel(), test_resin());
    let mut args = vec![
        model.to_str().expect("a UTF-8 path"),
        "--profile",
        panel.to_str().expect("a UTF-8 path"),
        "--material",
        resin.to_str().expect("a UTF-8 path"),
        "-o",
        out.to_str().expect("a UTF-8 path"),
    ];
    args.extend_from_slice(extra);
    let output = slice(&args);
    assert!(output.status.success(), "{}", stderr(&output));
    out
}

#[test]
fn a_model_sliced_plain_draws_the_layers_its_written_project_draws() {
    let model = write_box_stl("plain-vs-project.stl", 10.0, 12, 0);
    let project = output_file("plain-vs-project.encrust");
    let project_arg = project.to_str().expect("a UTF-8 path");

    let plain = sliced_plain(&model, "plain-stack", &[]);
    let through = sliced_plain(&model, "project-stack", &["--project-out", project_arg]);

    let layers = written_layers(&plain);
    assert_eq!(layers, 200, "a 10 mm cube at 0.05 mm is 200 layers");
    assert_eq!(
        written_layers(&through),
        layers,
        "the same number of layers"
    );
    for index in 0..layers {
        assert!(
            layer_pixels(&plain, index) == layer_pixels(&through, index),
            "layer {index} differs between the model and its project"
        );
    }
}

#[test]
fn a_copy_made_by_an_edit_counts_as_its_models_and_the_next_model_keeps_its_own() {
    let first = write_box_stl("copied-first", 10.0, 12, 0);
    let second = write_box_stl("copied-second", 10.0, 12, 0);
    let edits = edits_file(
        "copied.json",
        r#"[{"edit": "duplicate", "object": 1, "offset_mm": [40.0, 0.0, 0.0]}]"#,
    );

    let output = estimate(&[
        first.to_str().expect("a UTF-8 path"),
        second.to_str().expect("a UTF-8 path"),
        "--edits",
        edits.to_str().expect("a UTF-8 path"),
        "--json",
    ]);

    let report = json(&output);
    let meets = &report["models"][1]["meets"];
    assert_eq!(
        meets[0].as_str(),
        first.to_str(),
        "the second cube still stands in the first, its copy 40 mm off: {meets}"
    );
}

#[test]
fn two_projects_are_laid_on_one_plate_and_spread_apart() {
    let panel = project_fixture().with_file_name("project-panel.toml");
    let resin = test_resin();
    let out = output_file("two-projects.encrust");
    let output = estimate(&[
        project_fixture().to_str().expect("a UTF-8 path"),
        project_fixture().to_str().expect("a UTF-8 path"),
        "--arrange",
        "--profile",
        panel.to_str().expect("a UTF-8 path"),
        "--material",
        resin.to_str().expect("a UTF-8 path"),
        "--project-out",
        out.to_str().expect("a UTF-8 path"),
        "--json",
    ]);

    assert!(output.status.success(), "{}", stderr(&output));
    let project = load(&out).expect("the plate written reads back");
    let objects = &project.manifest.objects;
    assert_eq!(objects.len(), 2, "a cube from each project");
    assert_ne!(objects[0].id, objects[1].id, "each its own id");
    let apart = (objects[0].transform.translation - objects[1].transform.translation).length();
    assert!(
        apart >= 20.0,
        "two 20 mm cubes spread apart, {apart} mm between them"
    );
    assert!(
        !json(&output).to_string().contains("\"meets\""),
        "neither stands in the other"
    );
}
