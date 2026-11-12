//! `--drain-at auto`: holes the engine proves drain every trapped pocket, listed in `--json`
//! beside `slicing.trapped` (ADR 0401).

use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::support::*;

/// A 16 mm cube on the plate, hollowed to a 2 mm wall, sliced with `extra` and reported as JSON.
fn hollow_cube(name: &str, extra: &[&str]) -> Value {
    let model = write_box_stl(name, 16.0, 12, 0);
    let mut args = vec!["--hollow", "2", "--layer-height", "0.5"];
    args.extend_from_slice(extra);
    sliced(&model, name, &args)
}

/// `model` sliced on the tests' panel into a directory named for `name`, with `extra`, reported
/// as JSON.
fn sliced(model: &Path, name: &str, extra: &[&str]) -> Value {
    let out = output_dir(name);
    let panel = test_panel();
    let mut args = vec![
        "slice",
        model.to_str().unwrap(),
        "--profile",
        panel.to_str().unwrap(),
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

fn holes_of(report: &Value) -> &Vec<Value> {
    report["auto_drains"]["holes"]
        .as_array()
        .unwrap_or_else(|| panic!("auto_drains lists its holes: {report}"))
}

fn z_of(hole: &Value) -> f64 {
    hole["at_mm"][2]
        .as_f64()
        .expect("a hole's mouth is a point")
}

#[test]
fn auto_drains_empty_a_hollow_cube_through_a_low_drain_and_a_high_vent() {
    let report = hollow_cube("auto-drains", &["--drain-at", "auto"]);
    assert_eq!(
        report["status"], "ok",
        "drained, the run is clean: {report}"
    );
    assert_eq!(report["slicing"]["trapped_pockets"], 0);
    let pockets = report["auto_drains"]["pockets"]
        .as_array()
        .expect("pockets");
    assert_eq!(pockets.len(), 1, "one cavity: {pockets:?}");
    assert_eq!(pockets[0]["drained"], true);

    let holes = holes_of(&report);
    let role = |name: &str| holes.iter().find(|hole| hole["role"] == name);
    let drain = role("drain").expect("a drain");
    let vent = role("vent").expect("a vent");
    // The cavity's floor is the 2 mm wall; a 3 mm drain stands its radius over it.
    assert!(
        z_of(drain) - 1.5 <= 2.0 + 2.0,
        "the drain's lowest point is within a wall of the 2 mm floor, found {}",
        z_of(drain)
    );
    assert!(
        z_of(vent) > 16.0 - 2.0,
        "the vent opens through the 2 mm lid under 16 mm, found {}",
        z_of(vent)
    );
    assert_eq!(
        (drain["pocket"].as_u64(), drain["diameter_mm"].as_f64()),
        (Some(0), Some(3.0))
    );
}

#[test]
fn no_drain_vent_leaves_the_drain_alone() {
    let report = hollow_cube(
        "auto-drains-no-vent",
        &["--drain-at", "auto", "--no-drain-vent"],
    );
    let holes = holes_of(&report);
    assert_eq!(holes.len(), 1, "a drain and no vent: {holes:?}");
    assert_eq!(holes[0]["role"], "drain");
    assert_eq!(report["status"], "ok");
}

#[test]
fn a_pocket_no_hole_fits_stays_trapped_and_the_run_unclean() {
    // A 20 mm hole is wider than every 16 mm face of the cube.
    let report = hollow_cube(
        "auto-drains-too-wide",
        &["--drain-at", "auto", "--drain", "20"],
    );
    assert!(holes_of(&report).is_empty(), "no hole fits: {report}");
    assert_eq!(report["auto_drains"]["pockets"][0]["drained"], false);
    assert_eq!(report["slicing"]["trapped_pockets"], 1);
    assert_eq!(report["status"], "unclean");
}

#[test]
fn no_drain_vent_without_auto_is_refused() {
    let model = write_box_stl("auto-drains-refused", 8.0, 12, 0);
    let output = encrust(&[
        "estimate",
        model.to_str().unwrap(),
        "--hollow",
        "2",
        "--no-drain-vent",
    ]);
    assert!(!output.status.success(), "there is no vent to leave out");
    assert!(stderr(&output).contains("--no-drain-vent"));
}

#[test]
fn a_trapped_point_the_report_gives_drains_the_pocket_fed_back_to_drain_at() {
    let model = write_boxes_stl("drain-round-trip", &[([0.0; 3], [20.0, 20.0, 7.0])]);
    let shaped = [
        "--hollow",
        "2",
        "--supports",
        "medium",
        "--layer-height",
        "0.5",
    ];
    let (sealed, drilled) = (
        output_dir("drain-round-trip-found"),
        output_dir("drain-round-trip-drilled"),
    );
    let found = sliced(&model, "drain-round-trip-found", &shaped);
    let lifted_mm = found["supports"]["lifted_mm"].as_f64().expect("a lift");
    assert!(
        (lifted_mm - 2.0).abs() < 1e-4,
        "medium stands a box on the plate 2 mm up, got {lifted_mm}"
    );
    let at: Vec<String> = found["slicing"]["trapped"][0]["at_mm"]
        .as_array()
        .unwrap_or_else(|| panic!("the sealed box traps its cavity: {found}"))
        .iter()
        .map(|axis| axis.as_f64().expect("a coordinate").to_string())
        .collect();
    let at = at.join(",");

    let mut with_hole = shaped.to_vec();
    with_hole.extend(["--drain-at", &at]);
    let report = sliced(&model, "drain-round-trip-drilled", &with_hole);
    assert_eq!(
        report["slicing"]["trapped_pockets"], 0,
        "a hole at {at}, where the report put the pocket, drains it: {report}"
    );
    // The pocket's floor is nearest the box's underside; its lid is 4.75 mm off, untouched.
    let top = written_layers(&sealed) - 1;
    assert_eq!(
        lit_pixels(&drilled, top),
        lit_pixels(&sealed, top),
        "the hole goes in through the floor under the pocket, not through the lid"
    );
}

#[test]
fn a_point_placed_for_a_lift_the_supports_do_not_give_is_refused() {
    // A mould's 2 mm wall grows under the box, so medium stands it 7 mm up rather than 5.
    let model = write_box_stl("drain-mould-lift", 16.0, 12, 0);
    let output = encrust(&[
        "estimate",
        model.to_str().unwrap(),
        "--hollow",
        "2",
        "--hollow-mode",
        "external",
        "--supports",
        "medium",
        "--drain-at",
        "8,8,5",
    ]);
    assert!(!output.status.success(), "the point would land 2 mm off");
    assert!(stderr(&output).contains("planned"), "{}", stderr(&output));
}

#[test]
fn a_profile_that_lifts_nothing_keeps_the_drain_off_the_face_on_the_plate() {
    let profile = std::env::temp_dir().join("encrust-cli-tests/flush-supports.toml");
    fs::create_dir_all(profile.parent().expect("a temp file has a directory")).expect("temp dir");
    fs::write(&profile, "name = \"Flush\"\nz_lift_mm = 0.0\n").expect("write the profile");
    let report = hollow_cube(
        "auto-drains-flush",
        &[
            "--support-profile",
            profile.to_str().unwrap(),
            "--drain-at",
            "auto",
            "--no-drain-vent",
        ],
    );
    let holes = holes_of(&report);
    assert_eq!(holes.len(), 1, "one drain: {report}");
    // A 3 mm hole drilled level stands its 1.5 mm radius over its lowest point.
    assert!(
        z_of(&holes[0]) - 1.5 >= 1.0 - 1e-3,
        "the drain's lowest point stands a millimetre over the plate it rests on, found {}",
        z_of(&holes[0])
    );
}

#[test]
fn an_auto_drain_into_a_thin_cavity_leaves_the_top_whole() {
    let model = write_boxes_stl("auto-drains-thin", &[([0.0; 3], [16.0, 16.0, 4.0])]);
    let shaped = [
        "--hollow",
        "1.5",
        "--supports",
        "medium",
        "--layer-height",
        "0.25",
    ];
    let (sealed, drilled) = (
        output_dir("auto-drains-thin-sealed"),
        output_dir("auto-drains-thin"),
    );
    sliced(&model, "auto-drains-thin-sealed", &shaped);
    let mut drained = shaped.to_vec();
    drained.extend(["--drain-at", "auto", "--no-drain-vent"]);
    let report = sliced(&model, "auto-drains-thin", &drained);
    assert!(
        !holes_of(&report).is_empty(),
        "a drain is drilled: {report}"
    );

    let layers = written_layers(&sealed);
    assert_eq!(written_layers(&drilled), layers);
    // The 1.5 mm lid is six 0.25 mm layers; the top two lie wholly over the 1 mm cavity's ceiling.
    for layer in layers - 2..layers {
        assert_eq!(
            lit_pixels(&drilled, layer),
            lit_pixels(&sealed, layer),
            "layer {layer} of the lid is as whole as without a hole"
        );
    }
}
