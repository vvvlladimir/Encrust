//! `--prepare`: the autopilot's defaults filled in where no flag was typed, and the report of
//! what it decided (ADR 0399).

use std::path::{Path, PathBuf};

use core_geometry::{Mesh, Vec3, cuboid};
use serde_json::Value;

use crate::support::*;

/// The panel that lights its whole 80 mm build volume; `test-panel.toml` would clip a 40 mm
/// cube centred on it.
fn whole_panel() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/project-panel.toml")
}

/// A box of `size` mm standing on the plate's origin corner.
fn box_of(name: &str, size: Vec3) -> PathBuf {
    write_mesh_stl(name, &cuboid(Vec3::ZERO, size))
}

/// `model` sliced under `--prepare` with `extra`, centred on the whole panel, as JSON.
fn prepared(model: &Path, name: &str, extra: &[&str]) -> Value {
    let out = output_dir(name);
    let panel = whole_panel();
    let mut args = vec![
        "slice",
        model.to_str().unwrap(),
        "--profile",
        panel.to_str().unwrap(),
        "--layer-height",
        "0.5",
        "--center",
        "--prepare",
        "-o",
        out.to_str().unwrap(),
        "--json",
    ];
    args.extend_from_slice(extra);
    let output = encrust(&args);
    assert!(
        output.status.success(),
        "the run failed:\n{}",
        stderr(&output)
    );
    json(&output)
}

#[test]
fn a_large_solid_cube_is_hollowed_drained_and_supported_and_prints_clean() {
    let report = prepared(
        &box_of("prepare-cube", Vec3::splat(40.0)),
        "prepare-cube",
        &[],
    );
    let decided = &report["prepared"];
    assert_eq!(decided["repaired"], false, "a closed cube has no defect");
    assert_eq!(decided["oriented"], true);
    assert_eq!(decided["hollowed"]["wall_mm"], 2.0, "Hollow's default wall");
    let saves = decided["hollowed"]["saves_mm3"].as_f64().expect("a saving");
    // 40³ less its 6 · 40² surface times the 2 mm wall.
    let expected = 64_000.0 - 9_600.0 * 2.0;
    assert!(
        (saves - expected).abs() < 1.0,
        "saves {expected} mm³, found {saves}"
    );
    assert_eq!(decided["drained"], true);
    assert_eq!(decided["supports"], "Auto");

    assert_eq!(report["hollow"]["wall_mm"], 2.0);
    assert_eq!(report["slicing"]["trapped_pockets"], 0, "no resin trapped");
    assert!(
        report["supports"]["contacts"].as_u64() > Some(0),
        "the cavity's ceiling is held up: {report}"
    );
    assert_eq!(report["status"], "ok", "{report}");
}

#[test]
fn a_thin_plate_is_left_solid_and_undrained() {
    let model = box_of("prepare-plate", Vec3::new(60.0, 60.0, 3.0));
    let panel = whole_panel();
    let output = estimate(&[
        model.to_str().unwrap(),
        "--profile",
        panel.to_str().unwrap(),
        "--center",
        "--prepare",
        "--json",
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    let model = &json(&output)["models"][0];
    assert_eq!(model["prepared"]["hollowed"], Value::Null, "{model}");
    assert_eq!(model["prepared"]["drained"], false);
    assert_eq!(model["hollow"], Value::Null, "nothing was hollowed");
    assert_eq!(model["auto_drains"], Value::Null);
}

#[test]
fn flags_for_a_hollow_the_model_did_not_get_are_set_aside() {
    let model = box_of("prepare-idle", Vec3::new(60.0, 60.0, 3.0));
    let panel = whole_panel();
    let output = estimate(&[
        model.to_str().unwrap(),
        "--profile",
        panel.to_str().unwrap(),
        "--center",
        "--prepare",
        "--no-drain-vent",
        "--infill",
        "hive",
        "--json",
    ]);
    assert!(
        output.status.success(),
        "without --prepare both would be refused as idle: {}",
        stderr(&output)
    );
    assert_eq!(
        json(&output)["models"][0]["prepared"]["hollowed"],
        Value::Null
    );
}

#[test]
fn a_hollow_flag_wins_over_the_default_wall() {
    let report = prepared(
        &box_of("prepare-wall", Vec3::splat(40.0)),
        "prepare-wall",
        &["--hollow", "3"],
    );
    let hollowed = &report["prepared"]["hollowed"];
    assert_eq!(hollowed["wall_mm"], 3.0, "{report}");
    assert_eq!(
        hollowed["saves_mm3"],
        Value::Null,
        "the wall was not chosen"
    );
    assert_eq!(report["hollow"]["wall_mm"], 3.0);
    assert_eq!(report["prepared"]["drained"], true, "still drained");
}

#[test]
fn a_supports_flag_wins_over_the_automatic_profile() {
    let report = prepared(
        &box_of("prepare-light", Vec3::new(60.0, 60.0, 3.0)),
        "prepare-light",
        &["--supports", "light"],
    );
    assert_eq!(report["prepared"]["supports"], "Light", "{report}");
}

#[test]
fn a_rotation_given_is_kept_and_not_searched() {
    let report = prepared(
        &box_of("prepare-rotated", Vec3::new(30.0, 20.0, 10.0)),
        "prepare-rotated",
        &["--rotate", "0,90,0"],
    );
    assert_eq!(report["prepared"]["oriented"], false, "{report}");
    assert_eq!(report["oriented"], Value::Null);
    let size = &report["model"]["size_mm"];
    let near = |axis: usize, mm: f64| (size[axis].as_f64().expect("a size") - mm).abs() < 1e-3;
    assert!(
        near(0, 10.0) && near(2, 30.0),
        "turned 90° about y, the 30 mm side stands up: {size}"
    );
}

#[test]
fn a_model_with_a_hole_is_repaired_first() {
    let mut open: Mesh = cuboid(Vec3::ZERO, Vec3::splat(10.0));
    open.faces.pop();
    let report = prepared(&write_mesh_stl("prepare-open", &open), "prepare-open", &[]);
    assert_eq!(report["prepared"]["repaired"], true, "{report}");
    assert_eq!(report["repair"]["holes_closed"], 1);
    assert_eq!(report["repair"]["closed"], true);
}

#[test]
fn a_project_refuses_prepare() {
    let project = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cube.encrust");
    let out = output_dir("prepare-project");
    let output = slice(&[
        project.to_str().unwrap(),
        "--prepare",
        "-o",
        out.to_str().unwrap(),
    ]);
    assert!(!output.status.success(), "a project carries its own steps");
    assert!(stderr(&output).contains("--prepare"), "{}", stderr(&output));
}
