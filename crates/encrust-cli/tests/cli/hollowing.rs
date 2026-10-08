//! Hollowing a model from the command line: the cavity, its infill, the drain holes
//! drilled into it and the resin it would trap without them.

use std::path::PathBuf;

use crate::support::*;

/// A 16 mm cube sliced at 1 mm, with whatever hollowing arguments follow.
fn hollow_cube(name: &str, extra: &[&str]) -> (PathBuf, String) {
    let path = write_box_stl(&format!("hollow-{name}"), 16.0, 12, 0);
    let out = output_dir(name);
    let mut args = vec![
        path.to_str().unwrap().to_owned(),
        "--profile".to_owned(),
        test_panel().to_str().unwrap().to_owned(),
        "--layer-height".to_owned(),
        "1".to_owned(),
        "-o".to_owned(),
        out.to_str().unwrap().to_owned(),
    ];
    args.extend(extra.iter().map(|argument| (*argument).to_owned()));

    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = slice(&borrowed);
    assert!(
        output.status.success(),
        "the run failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    (out, stdout(&output))
}

#[test]
fn a_hollowed_cube_exposes_a_ring_on_the_layers_that_cross_its_cavity() {
    let (out, _) = hollow_cube("ring", &["--hollow", "3"]);

    // Halfway up, a 16 mm cube with a 3 mm wall is a 16 mm square with a 10 mm square out
    // of the middle: 156 mm2, which is 15 600 pixels at a 0.1 mm pitch.
    let ring = lit_pixels(&out, 8);
    assert!(
        (ring as f32 - 15_600.0).abs() / 15_600.0 < 0.02,
        "a 16 mm ring around a 10 mm cavity is 15 600 pixels, got {ring}"
    );

    // The floor of an internal hollow is solid, so the first layer is the whole square.
    let floor = lit_pixels(&out, 0);
    assert!(
        (floor as f32 - 25_600.0).abs() / 25_600.0 < 0.02,
        "the floor of an internal hollow is a whole 16 mm square, got {floor}"
    );
}

#[test]
fn hollowing_reports_the_resin_it_saves() {
    let (_, text) = hollow_cube("saved", &["--hollow", "3"]);

    assert_eq!(field(&text, "wall"), "3.000 mm, internal");
    let saved: f32 = field(&text, "resin saved")
        .trim_end_matches(" mm^3")
        .parse()
        .expect("the saved volume is a number");
    assert!(
        (saved - 1000.0).abs() / 1000.0 < 0.03,
        "a 10 mm cube of resin is 1000 mm^3, got {saved}"
    );
}

#[test]
fn infill_puts_material_back_into_the_cavity() {
    let (empty, _) = hollow_cube("empty", &["--hollow", "3"]);
    let (filled, text) = hollow_cube(
        "filled",
        &[
            "--hollow",
            "3",
            "--infill",
            "grid",
            "--infill-size",
            "4",
            "--infill-density",
            "0.3",
        ],
    );

    assert_eq!(
        field(&text, "infill"),
        "grid, 4.0 mm cells at 30%, 0.65 mm walls"
    );
    assert!(
        lit_pixels(&filled, 8) > lit_pixels(&empty, 8),
        "the grid must show up inside the cavity"
    );
}

#[test]
fn a_wall_thicker_than_the_model_is_reported_as_a_defect() {
    let (_, text) = hollow_cube("thick", &["--hollow", "12"]);
    assert!(
        text.contains("the wall left no room for a cavity"),
        "a 12 mm wall in a 16 mm cube hollows nothing:\n{text}"
    );
}

#[test]
fn a_drain_hole_opens_the_lid_of_a_hollow_box() {
    let path = write_box_stl("drained", 20.0, 12, 0);
    let input = path.to_str().unwrap();
    let panel = test_panel();
    let panel = panel.to_str().unwrap();

    let solid_out = output_dir("drain-closed");
    let solid = slice(&[
        input,
        "--profile",
        panel,
        "--hollow",
        "2",
        "--layer-height",
        "0.5",
        "-o",
        solid_out.to_str().unwrap(),
    ]);
    assert!(solid.status.success(), "{}", stdout(&solid));

    let drained_out = output_dir("drain-open");
    let drained = slice(&[
        input,
        "--profile",
        panel,
        "--hollow",
        "2",
        "--layer-height",
        "0.5",
        "--drain",
        "6",
        "--drain-depth",
        "4",
        "--drain-at",
        "10,10,25",
        "-o",
        drained_out.to_str().unwrap(),
    ]);
    let text = stdout(&drained);
    assert!(drained.status.success(), "{text}");
    assert_eq!(field(&text, "drains"), "1 hole(s), 0 channel(s)");

    // The lid of the box, which is solid without the hole and a frame with it. The box
    // stands from the plate's origin, so its middle is the centre of the 200 px panel.
    let top = written_layers(&solid_out) - 2;
    let (width, height, closed) = layer_pixels(&solid_out, top);
    let (_, _, open) = layer_pixels(&drained_out, top);
    let middle = (height as usize / 2) * width as usize + width as usize / 2;

    assert_eq!(
        closed[middle], 0xFF,
        "the lid is solid without a hole in it"
    );
    assert_eq!(open[middle], 0x00, "the hole goes right through the lid");
    let lit = |pixels: &[u8]| pixels.iter().filter(|&&pixel| pixel > 0).count();
    assert!(lit(&open) < lit(&closed), "and takes resin with it");
}

#[test]
fn a_drain_hole_is_cut_into_a_solid_model_too() {
    let path = write_box_stl("solid-drain", 20.0, 12, 0);
    let input = path.to_str().unwrap();
    let panel = test_panel();
    let panel = panel.to_str().unwrap();

    let out = output_dir("solid-drain");
    let drilled = slice(&[
        input,
        "--profile",
        panel,
        "--layer-height",
        "0.5",
        "--drain",
        "6",
        "--drain-depth",
        "4",
        "--drain-at",
        "10,10,25",
        "-o",
        out.to_str().unwrap(),
    ]);
    let text = stdout(&drilled);
    assert!(drilled.status.success(), "{text}");
    assert_eq!(field(&text, "drains"), "1 hole(s), 0 channel(s)");

    // No --hollow anywhere: the hole is cut into the solid all the same, four millimetres
    // down from the lid.
    let top = written_layers(&out) - 2;
    let (width, height, lid) = layer_pixels(&out, top);
    let middle = (height as usize / 2) * width as usize + width as usize / 2;
    assert_eq!(lid[middle], 0x00, "the hole is open at the top of the box");

    let floor = written_layers(&out) / 2;
    let (_, _, half_way) = layer_pixels(&out, floor);
    assert_eq!(
        half_way[middle], 0xFF,
        "and stops at its own depth rather than going through the box"
    );
}

#[test]
fn a_hollow_box_with_no_hole_reports_the_resin_it_traps() {
    let path = write_box_stl("trapped", 20.0, 12, 0);
    let input = path.to_str().unwrap();
    let sealed = stdout(&estimate(&[
        input,
        "--hollow",
        "2",
        "--layer-height",
        "0.5",
    ]));

    let reported = field(&sealed, "trapped resin");
    let held: f32 = reported
        .split_whitespace()
        .next()
        .and_then(|number| number.parse().ok())
        .unwrap_or_else(|| panic!("no volume in {reported:?}"));
    let cavity: f32 = field(&sealed, "resin saved")
        .split_whitespace()
        .next()
        .and_then(|number| number.parse().ok())
        .expect("the hollow report states what it took out");
    assert!(
        (held - cavity).abs() / cavity < 0.05,
        "the pocket holds the cavity the run cut: {held} against {cavity}"
    );

    let drained = stdout(&estimate(&[
        input,
        "--hollow",
        "2",
        "--layer-height",
        "0.5",
        "--drain",
        "4",
        "--drain-at",
        "10,10,20",
        "--drain-depth",
        "3",
    ]));
    assert!(
        !drained.contains("trapped resin"),
        "a hole through the lid lets it all out:\n{drained}"
    );
}

#[test]
fn a_hive_in_a_cavity_drains_through_one_hole() {
    // Each cell used to be a sealed box, so a hole in the lid drained the one under it and
    // left every other cell full.
    let path = write_box_stl("hive-drain", 20.0, 12, 0);
    let input = path.to_str().unwrap();
    let report = stdout(&estimate(&[
        input,
        "--hollow",
        "2",
        "--infill",
        "hive",
        "--infill-size",
        "5",
        "--infill-density",
        "0.15",
        "--layer-height",
        "0.5",
        "--drain",
        "4",
        "--drain-at",
        "10,10,20",
        "--drain-depth",
        "3",
    ]));
    assert!(
        !report.contains("trapped resin"),
        "the cells are open over the floor of the cavity, so one hole drains all of \
         them:\n{report}"
    );
}

#[test]
fn trapped_resin_fails_a_strict_run() {
    let path = write_box_stl("trapped-strict", 20.0, 12, 0);
    let output = estimate(&[
        path.to_str().unwrap(),
        "--hollow",
        "2",
        "--layer-height",
        "0.5",
        "--strict",
    ]);

    assert_eq!(
        output.status.code(),
        Some(3),
        "resin that cannot get out is a defect:\n{}",
        stdout(&output)
    );
}

#[test]
fn a_solid_model_is_not_checked_for_drainage_unless_it_is_asked_for() {
    let path = write_box_stl("drainage-flag", 10.0, 12, 0);
    let input = path.to_str().unwrap();

    let quiet = stdout(&estimate(&[input, "--layer-height", "1"]));
    assert!(!quiet.contains("trapped resin"));

    let checked = stdout(&estimate(&[
        input,
        "--layer-height",
        "1",
        "--check-drainage",
    ]));
    assert!(
        !checked.contains("trapped resin"),
        "a solid box traps nothing either way"
    );
}

#[test]
fn a_hole_shallower_than_the_wall_still_goes_through_it() {
    let path = write_box_stl("shallow-drain", 20.0, 12, 0);
    let out = output_dir("shallow-drain");
    let panel = test_panel();
    let output = slice(&[
        path.to_str().unwrap(),
        "--profile",
        panel.to_str().unwrap(),
        "--hollow",
        "2",
        "--layer-height",
        "0.5",
        // Half a millimetre into a two millimetre wall, drilled into the side of the box.
        "--drain",
        "4",
        "--drain-depth",
        "0.5",
        "--drain-at",
        "0,10,10",
        "-o",
        out.to_str().unwrap(),
    ]);
    assert!(output.status.success(), "{}", stdout(&output));

    // The layer through the middle of the hole, along the row the hole is centred on: the
    // box stands from the plate's origin on a 0.1 mm panel.
    let (width, height, pixels) = layer_pixels(&out, 20);
    let row = height as usize / 2;
    let start = row * width as usize;
    let wall: Vec<u8> = pixels[start..start + 30].to_vec();

    assert!(
        wall.iter().all(|pixel| *pixel == 0),
        "the hole clears the whole 2 mm wall in front of it, got {wall:?}"
    );
}
