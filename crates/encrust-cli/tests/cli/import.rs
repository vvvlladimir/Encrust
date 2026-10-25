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
fn repair_closes_the_hole_an_open_box_has() {
    let path = write_box_stl("repaired", 10.0, 10, 0);
    let output = encrust(&["inspect", path.to_str().unwrap(), "--repair"]);
    let text = stdout(&output);

    assert!(output.status.success());
    assert_eq!(field(&text, "repair"), "closed 1 hole");
    assert_eq!(field(&text, "closed"), "yes (1 shell, euler 2)");
    assert_eq!(field(&text, "volume"), "1000.000 mm^3");
}

#[test]
fn repair_says_in_json_how_many_holes_it_closed() {
    let path = write_box_stl("repaired-json", 10.0, 10, 0);
    let report = json(&encrust(&[
        "--json",
        "inspect",
        path.to_str().unwrap(),
        "--repair",
    ]));

    assert_eq!(report["repair"]["holes_closed"], 1, "got {report}");
    assert_eq!(report["repair"]["holes_left"], 0);
    assert_eq!(report["repair"]["kept_as_found"], false);
    assert_eq!(report["repair"]["seams_stitched"], 0, "a box has no seam");
}

#[test]
fn close_small_holes_patches_a_missing_face_and_leaves_a_missing_side() {
    let one_face = write_box_stl("one-face-missing", 10.0, 11, 0);
    let one_side = write_box_stl("one-side-missing", 10.0, 10, 0);
    let run = |path: &std::path::Path| {
        stdout(&encrust(&[
            "inspect",
            path.to_str().unwrap(),
            "--repair=close-small-holes",
        ]))
    };

    assert_eq!(field(&run(&one_face), "repair"), "closed 1 hole");
    assert_eq!(field(&run(&one_side), "defect"), "4 open edges");
}

#[test]
fn a_seam_tolerance_without_repair_is_a_usage_error() {
    let path = write_box_stl("seam-unrepaired", 10.0, 10, 0);
    let output = encrust(&["inspect", path.to_str().unwrap(), "--seam-tolerance", "0.1"]);
    assert_eq!(
        output.status.code(),
        Some(2),
        "it does nothing without --repair"
    );
}

#[test]
fn repair_and_no_validate_cannot_be_asked_together() {
    let path = write_box_stl("repair-unvalidated", 10.0, 10, 0);
    let output = encrust(&[
        "inspect",
        path.to_str().unwrap(),
        "--repair",
        "--no-validate",
    ]);
    assert_eq!(output.status.code(), Some(2), "a usage error");
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

/// A checkerboard picture of 8 x 8 grey pixels, written beside the test's other files.
fn write_checkers(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("encrust-cli-{name}.png"));
    let file = std::fs::File::create(&path).expect("the temp dir is writable");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), 8, 8);
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(png::BitDepth::Eight);
    let pixels: Vec<u8> = (0..64u8)
        .map(|i| if (i / 8 + i % 8) % 2 == 0 { 255 } else { 0 })
        .collect();
    let mut writer = encoder.write_header().expect("a valid header");
    writer.write_image_data(&pixels).expect("64 pixels");
    path
}

#[test]
fn a_model_with_no_uvs_takes_a_tiled_picture_as_relief() {
    let path = write_box_stl("tiled-relief", 10.0, 12, 0);
    let picture = write_checkers("tiled-relief");
    let output = slice(&[
        path.to_str().unwrap(),
        "--relief",
        "0.4",
        "--relief-image",
        picture.to_str().unwrap(),
        "--relief-tile",
        "4",
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        stdout(&output).contains("tiled every 4 mm"),
        "{}",
        stdout(&output)
    );

    let refused = slice(&[path.to_str().unwrap(), "--relief", "0.4"]);
    assert_eq!(
        refused.status.code(),
        Some(1),
        "an STL carries no map of its own"
    );
}

#[test]
fn the_relief_reading_and_holding_flags_reach_the_press() {
    let path = write_box_stl("held-relief", 10.0, 12, 0);
    let picture = write_checkers("held-relief");
    let with = |hold_bottom: &str| {
        slice(&[
            path.to_str().unwrap(),
            "--relief",
            "0.4",
            "--relief-image",
            picture.to_str().unwrap(),
            "--relief-invert",
            "--relief-symmetric",
            "--relief-smooth",
            "1.5",
            "--relief-hold-top",
            "5",
            "--relief-hold-bottom",
            hold_bottom,
        ])
    };
    let held = with("5");
    assert!(held.status.success(), "{}", stderr(&held));

    let refused = with("95");
    assert_eq!(
        refused.status.code(),
        Some(1),
        "a face cannot be past level: {}",
        stderr(&refused)
    );
}

#[test]
fn an_orient_goal_without_orient_is_an_argument_error() {
    let path = write_box_stl("goal-alone", 10.0, 12, 0);
    let output = slice(&[path.to_str().unwrap(), "--orient-goal", "least-peel"]);
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
}

#[test]
fn the_orient_goal_and_the_tilt_limit_reach_the_search() {
    // A cube ties on every goal and stays; a leaning box does not.
    let path = write_leaning_box_stl("no-tilt", 10.0, 8.0, 12, 0);
    let turned = |goal: &str, extra: &[&str]| {
        let base = [
            path.to_str().unwrap(),
            "--orient",
            "--orient-goal",
            goal,
            "--json",
        ];
        let output = slice(&[&base[..], extra].concat());
        assert!(output.status.success(), "{}", stderr(&output));
        json(&output)["oriented"]["degrees"]
            .as_f64()
            .expect("an oriented run reports its turn")
    };

    let shortest = turned("shortest-print", &[]);
    assert!(
        (shortest - turned("fewest-supports", &[])).abs() > 1.0,
        "the shortest print lays the box otherwise than the fewest supports stand it"
    );
    assert!(turned("auto", &[]).is_finite(), "auto reaches the search");
    assert!(shortest > 1.0, "the shortest print turns the box");
    assert_eq!(
        turned("shortest-print", &["--max-tilt", "0"]),
        0.0,
        "a tilt of nothing allows no turn"
    );
}

#[test]
fn the_overhang_an_orientation_is_scored_on_is_the_support_profiles_own() {
    // A wall hanging 50 degrees from vertical: past the 40 the medium profile holds up
    // from, inside the 55 a profile of its own asks for.
    let lean = 10.0 * 50.0_f32.to_radians().tan();
    let path = write_leaning_box_stl("leaning", 10.0, lean, 12, 0);
    let medium = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/profiles/supports/medium.toml"
    ))
    .expect("the shipped medium profile");
    let lenient = std::env::temp_dir()
        .join("encrust-cli-tests")
        .join("lenient.toml");
    std::fs::write(
        &lenient,
        medium.replace("max_overhang_deg = 40.0", "max_overhang_deg = 55.0"),
    )
    .expect("write the lenient profile");
    let overhang = |profile: &[&str]| {
        let mut args = vec![
            path.to_str().unwrap(),
            "--orient",
            "--max-tilt",
            "0",
            "--json",
        ];
        args.extend_from_slice(profile);
        let output = slice(&args);
        assert!(output.status.success(), "{}", stderr(&output));
        json(&output)["oriented"]["overhang_mm2"]
            .as_f64()
            .expect("an oriented run reports its overhang")
    };

    // 155.6 mm2 of wall, weighted by how far past the angle it leans: sin 50 - sin 40.
    let held = overhang(&["--supports", "medium"]);
    assert!(
        held > 5.0,
        "the wall is 19 mm2 of weighted overhang at 40 degrees, got {held}"
    );
    let lenient = overhang(&["--support-profile", lenient.to_str().unwrap()]);
    assert!(lenient < 0.01, "and none at 55 degrees, got {lenient}");
}

#[test]
fn a_model_set_on_the_plate_with_a_ledge_hanging_is_stood_on_supports() {
    // A 20 mm cube with a 4 mm ledge out of its top: too little of its surface to keep it
    // off the plate, enough to need holding up there.
    let path = write_boxes_stl(
        "ledge",
        &[
            ([0.0, 0.0, 0.0], [20.0, 20.0, 20.0]),
            ([20.0, 8.0, 18.0], [24.0, 12.0, 20.0]),
        ],
    );
    let output = slice(&[
        path.to_str().unwrap(),
        "--orient",
        "--max-tilt",
        "0",
        "--supports",
        "medium",
        "--json",
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    let report = json(&output);
    assert!(
        report["supports"]["contacts"].as_u64().unwrap_or(0) > 0,
        "the ledge is held up: {report}"
    );
}

#[test]
fn an_oriented_model_stands_on_the_plate_without_centring() {
    // Turned over about the file's origin, a box drawn on the plate ends up under it.
    let path = write_leaning_box_stl("turned-over", 10.0, 8.0, 12, 0);
    let out = output_dir("turned-over");
    let output = slice(&[
        path.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--profile",
        shipped_profile().to_str().unwrap(),
        "--orient",
        "--orient-goal",
        "shortest-print",
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        !stderr(&output).contains("under the plate"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn supports_sized_by_what_they_hold_stand_from_the_command_line() {
    let path = write_boxes_stl(
        "ledge-auto",
        &[
            ([0.0, 0.0, 0.0], [20.0, 20.0, 20.0]),
            ([20.0, 8.0, 18.0], [24.0, 12.0, 20.0]),
        ],
    );
    let output = slice(&[path.to_str().unwrap(), "--supports", "auto", "--json"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let report = json(&output);
    assert!(
        report["supports"]["contacts"].as_u64().unwrap_or(0) > 0,
        "the automatic profile holds the model up: {report}"
    );
}

#[test]
fn a_cavity_is_held_up_from_inside_only_when_asked() {
    let path = write_box_stl("cavity", 20.0, 12, 0);
    let contacts = |more: &[&str]| {
        let mut args = vec![
            path.to_str().unwrap(),
            "--hollow",
            "2",
            "--supports",
            "medium",
            "--json",
        ];
        args.extend_from_slice(more);
        let output = slice(&args);
        assert!(output.status.success(), "{}", stderr(&output));
        json(&output)["supports"]["contacts"].as_u64().unwrap_or(0)
    };
    let outside = contacts(&[]);
    let inside = contacts(&["--support-cavity"]);
    assert!(
        inside > outside,
        "a 16 mm cavity has a flat ceiling to hold: {inside} contacts against {outside}"
    );
}

#[test]
fn holding_up_a_cavity_without_supports_is_refused() {
    let path = write_box_stl("cavity-unsupported", 10.0, 12, 0);
    let output = slice(&[path.to_str().unwrap(), "--hollow", "2", "--support-cavity"]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("--supports"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn holding_up_a_cavity_without_hollowing_is_refused() {
    let path = write_box_stl("no-cavity", 10.0, 12, 0);
    let output = slice(&[
        path.to_str().unwrap(),
        "--supports",
        "medium",
        "--support-cavity",
    ]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("--hollow"), "{}", stderr(&output));
}
