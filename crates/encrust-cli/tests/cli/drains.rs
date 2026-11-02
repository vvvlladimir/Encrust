//! `--drain-at auto`: holes the engine proves drain every trapped pocket, listed in `--json`
//! beside `slicing.trapped` (ADR 0396).

use serde_json::Value;

use crate::support::*;

/// A 16 mm cube on the plate, hollowed to a 2 mm wall, sliced with `extra` and reported as JSON.
fn hollow_cube(name: &str, extra: &[&str]) -> Value {
    let model = write_box_stl(name, 16.0, 12, 0);
    let out = output_dir(name);
    let panel = test_panel();
    let mut args = vec![
        "slice",
        model.to_str().unwrap(),
        "--profile",
        panel.to_str().unwrap(),
        "--layer-height",
        "0.5",
        "--hollow",
        "2",
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
