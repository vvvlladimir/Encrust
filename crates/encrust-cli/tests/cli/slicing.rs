//! Cutting a model into layers: what the stack adds up to, what a broken mesh is
//! reported as, and what the machine refuses to print.

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
    // Turned a quarter about X without --center, so the box hangs off the front edge.
    let path = write_box_stl("off-the-edge", 10.0, 12, 0);
    let output = estimate(&[
        path.to_str().unwrap(),
        "--rotate",
        "90,0,0",
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
