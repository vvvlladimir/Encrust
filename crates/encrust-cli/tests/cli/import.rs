//! Importing a model: what the report says about it, what repair changed, and the flags
//! that place it.

use crate::support::*;

#[test]
fn a_cube_is_welded_validated_and_reported() {
    let path = write_box_stl("cube", 10.0, 12, 0);
    let output = slice(&[path.to_str().unwrap()]);
    let text = stdout(&output);

    assert!(output.status.success());
    assert_eq!(field(&text, "vertices"), "8 (welded from 36)");
    assert_eq!(field(&text, "triangles"), "12");
    assert_eq!(field(&text, "size"), "10.000 x 10.000 x 10.000 mm");
    assert_eq!(field(&text, "volume"), "1000.000 mm^3");
    assert_eq!(field(&text, "closed"), "yes (1 shell, euler 2)");
    assert_eq!(field(&text, "orientation"), "consistent");
}

#[test]
fn inverted_faces_are_reported_as_fixed() {
    let path = write_box_stl("inverted", 10.0, 12, 3);
    let text = stdout(&slice(&[path.to_str().unwrap()]));
    assert_eq!(field(&text, "orientation"), "fixed 3 inverted faces");
}

#[test]
fn an_open_mesh_is_reported_but_still_succeeds() {
    let path = write_box_stl("open", 10.0, 10, 0);
    let output = slice(&[path.to_str().unwrap()]);
    let text = stdout(&output);

    assert!(
        output.status.success(),
        "a defect alone must not fail the run"
    );
    assert_eq!(
        field(&text, "closed"),
        "no (1 shell, 4 edges that do not close)"
    );
    assert_eq!(field(&text, "defect"), "4 open edges");
    assert!(
        !text
            .lines()
            .any(|line| line.trim_start().starts_with("volume")),
        "an open mesh encloses no defined volume"
    );
}

#[test]
fn inspect_stops_after_the_import_report() {
    let path = write_box_stl("no-slice", 10.0, 12, 0);
    let output = encrust(&["inspect", path.to_str().unwrap()]);
    let text = stdout(&output);

    assert!(output.status.success());
    assert!(!text.contains("layers"));
}

#[test]
fn trace_says_more_than_debug_does() {
    let path = write_box_stl("two-levels", 10.0, 12, 0);
    let run = |level: &str| {
        stderr(&slice(&[
            path.to_str().unwrap(),
            "--layer-height",
            "2",
            level,
        ]))
    };

    let debug = run("-v");
    let trace = run("-vv");
    assert!(
        !debug.contains("layer sliced") && trace.contains("layer sliced"),
        "-vv promises trace and has to deliver it\ndebug:\n{debug}\ntrace:\n{trace}"
    );
}

#[test]
fn strict_fails_on_an_open_mesh_and_passes_on_a_closed_one() {
    let open = write_box_stl("strict-open", 10.0, 10, 0);
    let closed = write_box_stl("strict-closed", 10.0, 12, 0);

    assert_eq!(
        slice(&[open.to_str().unwrap(), "--strict"]).status.code(),
        Some(3),
        "an unclean model under --strict is exit code 3"
    );
    assert!(
        slice(&[closed.to_str().unwrap(), "--strict"])
            .status
            .success()
    );
}

#[test]
fn scale_and_rotation_change_the_reported_size() {
    let path = write_box_stl("transformed", 10.0, 12, 0);
    let scaled = stdout(&slice(&[path.to_str().unwrap(), "--scale", "2,1,0.5"]));
    assert_eq!(field(&scaled, "size"), "20.000 x 10.000 x 5.000 mm");

    let rotated = stdout(&slice(&[path.to_str().unwrap(), "--rotate", "0,0,90"]));
    assert_eq!(field(&rotated, "size"), "10.000 x 10.000 x 10.000 mm");
    assert_eq!(field(&rotated, "volume"), "1000.000 mm^3");
}

#[test]
fn a_negative_number_after_a_flag_is_a_number_and_not_a_flag() {
    let path = write_box_stl("negative-angle", 10.0, 12, 0);
    let spaced = estimate(&[path.to_str().unwrap(), "--center", "--rotate", "-45,0,0"]);

    assert!(
        spaced.status.success(),
        "a turn back is an everyday rotation: {}",
        stderr(&spaced)
    );
    // A 10 mm cube turned 45 degrees about X stands root two taller and deeper.
    assert_eq!(
        field(&stdout(&spaced), "size"),
        "10.000 x 14.142 x 14.142 mm"
    );
    let attached = estimate(&[path.to_str().unwrap(), "--center", "--rotate=-45,0,0"]);
    assert_eq!(
        stdout(&spaced),
        stdout(&attached),
        "the two spellings of one flag say the same thing"
    );
}

#[test]
fn center_sits_the_model_on_the_plate() {
    let path = write_box_stl("centered", 10.0, 12, 0);
    let text = stdout(&estimate(&[
        path.to_str().unwrap(),
        "--center",
        "--profile",
        shipped_profile().to_str().unwrap(),
    ]));

    let bounds = field(&text, "bounds");
    assert!(
        bounds.starts_with("[71.680, 33.880, 0.000]"),
        "got {bounds}"
    );
    assert_eq!(field(&text, "fits"), "Mars 4 Ultra: yes");
}

#[test]
fn an_unknown_extension_is_an_error() {
    let output = slice(&["model.gcode"]);
    let stderr = String::from_utf8(output.stderr).expect("stderr is utf-8");

    assert_eq!(output.status.code(), Some(1), "a failed run is exit code 1");
    assert!(stderr.contains("gcode"), "got {stderr}");
}
