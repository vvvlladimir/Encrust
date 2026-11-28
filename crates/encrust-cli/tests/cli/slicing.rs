//! Cutting a model into layers: what the stack adds up to, what a broken mesh is
//! reported as, and what the machine refuses to print.

use std::fs;

use crate::support::*;

#[test]
fn a_cube_is_sliced_into_layers_that_add_back_up_to_its_volume() {
    let path = write_box_stl("sliced", 10.0, 12, 0);
    let out = output_dir("sliced-cube");
    let output = slice(&[
        path.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--layer-height",
        "0.1",
        "-o",
        out.to_str().unwrap(),
    ]);
    let text = stdout(&output);

    assert!(output.status.success());
    assert_eq!(field(&text, "layers"), "100");
    assert_eq!(field(&text, "contours"), "100 (up to 1 per layer)");
    // The resin figure is the masks', not the contours' (ADR 0206).
    let cured = field(&text, "cured volume")
        .trim()
        .trim_end_matches(" mm^3")
        .parse::<f32>()
        .expect("the cured volume is a number");
    assert!(
        (cured - 1000.0).abs() < 1.0,
        "a 10 mm cube cures 1000 mm^3: {cured}"
    );
    assert!(!text.contains("slice defect"));
}

#[test]
fn a_missing_wall_is_reported_as_a_slice_defect() {
    // Ten of the twelve faces: the box loses one of its four walls.
    let path = write_box_stl("strict-gap", 10.0, 10, 0);
    let output = slice(&[path.to_str().unwrap(), "--layer-height", "1"]);

    assert!(output.status.success());
    assert!(stdout(&output).contains("slice defect"));
}

#[test]
fn a_hole_in_the_mesh_is_counted_once_and_not_warned_about_per_layer() {
    let path = write_box_stl("hole-once", 10.0, 10, 0);
    let output = slice(&[path.to_str().unwrap(), "--layer-height", "0.5"]);

    assert!(output.status.success());
    assert!(
        !stderr(&output).contains("layer"),
        "twenty layers over one hole are one line of report, not twenty of log: {}",
        stderr(&output)
    );
    assert!(
        stdout(&output).contains("contours closed over a gap"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn a_model_small_enough_but_standing_off_the_plate_is_reported_by_estimate() {
    // Drawn in front of the plate's edge and not centred, so the box hangs off it.
    let path = write_boxes_stl("off-the-edge", &[([0.0, -10.0, 0.0], [10.0, 0.0, 10.0])]);
    let output = estimate(&[
        path.to_str().unwrap(),
        "--profile",
        shipped_profile().to_str().unwrap(),
        "--strict",
    ]);
    let text = stdout(&output);

    assert_eq!(
        field(&text, "fits"),
        "Mars 4 Ultra: no, off the plate in Y by 10.000 mm"
    );
    assert_eq!(
        output.status.code(),
        Some(3),
        "--strict has to notice what the panel would clip: {text}"
    );
}

#[test]
fn supports_reaching_off_the_plate_are_reported_though_the_model_fits() {
    // In the plate's corner, drawn over it, the block fits, but its raft reaches past it.
    let path = write_boxes_stl("raft-off-the-edge", &[([0.0, 0.0, 5.0], [12.0, 12.0, 9.0])]);
    let output = estimate(&[
        path.to_str().unwrap(),
        "--profile",
        shipped_profile().to_str().unwrap(),
        "--supports",
        "medium",
        "--strict",
    ]);
    let text = stdout(&output);
    let (imported, standing) = text
        .split_once("On the plate")
        .expect("an On the plate block");

    assert_eq!(field(imported, "fits"), "Mars 4 Ultra: yes");
    assert!(
        field(standing, "fits").starts_with("Mars 4 Ultra: no, off the plate in X by"),
        "{text}"
    );
    assert_eq!(
        output.status.code(),
        Some(3),
        "--strict has to notice the raft the panel would clip: {text}"
    );
}

#[test]
fn a_cube_on_the_plate_lifted_3_mm_is_stood_on_supports_there() {
    let path = write_box_stl("lifted-cube", 10.0, 12, 0);
    let output = estimate(&[
        path.to_str().unwrap(),
        "--supports",
        "medium",
        "--lift",
        "3",
        "--json",
    ]);
    let report = json(&output);
    let supports = &report["models"][0]["supports"];

    let lifted_mm = supports["lifted_mm"].as_f64().expect("a lift");
    assert!(
        (lifted_mm - 3.0).abs() < 1e-4,
        "the cube stands 3 mm up, got {lifted_mm}"
    );
    assert!(
        supports["trunks"].as_u64().is_some_and(|trunks| trunks > 0),
        "its floor is held up: {supports}"
    );
}

#[test]
fn a_model_larger_than_the_machine_is_reported() {
    let path = write_box_stl("oversized", 200.0, 12, 0);
    let output = estimate(&[
        path.to_str().unwrap(),
        "--profile",
        shipped_profile().to_str().unwrap(),
        "--strict",
    ]);

    assert_eq!(output.status.code(), Some(3));
    assert_eq!(
        field(&stdout(&output), "fits"),
        "Mars 4 Ultra: no, over X by 46.640 mm, Y by 122.240 mm, Z by 35.000 mm"
    );
}

#[test]
fn estimate_reports_the_stack_and_writes_nothing() {
    let path = write_box_stl("no-raster", 10.0, 12, 0);
    let text = stdout(&estimate(&[
        path.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--layer-height",
        "5",
    ]));

    assert!(text.contains("layers        2"));
    assert!(!text.contains("masks"), "no mask is written:\n{text}");
    // A 10 mm cube cures 1000 mm3, which is one millilitre.
    assert_eq!(field(&text, "resin"), "1.0 ml, 1.1 g", "{text}");
    assert!(text.contains("print time"));
}

#[test]
fn the_report_carries_the_fit_drift_of_a_resin_with_its_tolerance_off() {
    let path = write_box_stl("drift", 10.0, 12, 0);
    let mut resin = fs::read_to_string(test_resin()).expect("the fixture resin reads");
    resin.push_str(
        "\n[compensation]\ntolerance_on = false\nhole_offset_mm = 0.04\nouter_offset_mm = 0.03\n",
    );
    let resin_file = output_file("drift-resin.toml");
    fs::write(&resin_file, resin).expect("the resin is written");

    let output = estimate(&[
        path.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--material",
        resin_file.to_str().unwrap(),
        "--json",
    ]);
    let report: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("--json prints one report");

    let drift = report["slicing"]["fit_drift_mm"]
        .as_f64()
        .expect("a number");
    assert!(
        (drift - 0.07).abs() < 1e-6,
        "0.04 hole and 0.03 outer drift 0.07 a side, got {drift}"
    );
}

#[test]
fn an_exposure_typed_for_the_run_lengthens_the_print_over_the_resins() {
    let path = write_box_stl("exposed-cube", 10.0, 12, 0);
    let time_s = |extra: &[&str]| {
        let panel = test_panel().with_file_name("project-panel.toml");
        let resin = test_resin();
        let mut args = vec![
            path.to_str().unwrap(),
            "--profile",
            panel.to_str().unwrap(),
            "--material",
            resin.to_str().unwrap(),
            "--json",
        ];
        args.extend_from_slice(extra);
        json(&estimate(&args))["print"]["print_time_s"]
            .as_u64()
            .expect("a print time")
    };
    let own = time_s(&[]);
    let longer = time_s(&["--exposure", "9", "--bottom-layers", "2"]);
    assert!(
        longer > own,
        "9 s a layer takes longer than the resin's: {own} s, then {longer} s"
    );
}
