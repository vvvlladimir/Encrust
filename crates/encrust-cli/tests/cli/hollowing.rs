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
            "--infill-cell",
            "4",
            "--infill-density",
            "0.4",
        ],
    );

    // 4 mm cells 40% solid stand on a pitch of 4 / sqrt(0.6), 1.16 mm more than the cell.
    assert_eq!(
        field(&text, "infill"),
        "grid, 4.0 mm cells at 40%, 1.16 mm walls"
    );
    assert!(
        lit_pixels(&filled, 8) > lit_pixels(&empty, 8),
        "the grid must show up inside the cavity"
    );
}

#[test]
fn an_infill_size_with_no_infill_is_refused() {
    let path = write_box_stl("hollow-unused-size", 16.0, 12, 0);
    let out = output_dir("unused-size");
    for (flag, value) in [("--infill-cell", "6"), ("--infill-density", "0.2")] {
        let output = slice(&[
            path.to_str().unwrap(),
            "--profile",
            test_panel().to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--hollow",
            "3",
            flag,
            value,
        ]);
        assert!(
            !output.status.success(),
            "{flag} with no --infill is refused"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(flag),
            "and the refusal names the flag"
        );
    }
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
fn a_walled_hole_is_a_recess_that_drains_nothing() {
    let path = write_box_stl("walled-drain", 20.0, 12, 0);
    let report = |walled: &[&str]| {
        let mut args = vec![
            path.to_str().unwrap(),
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
        ];
        args.extend_from_slice(walled);
        stdout(&estimate(&args))
    };
    let drained = report(&[]);
    assert!(
        !drained.contains("trapped resin"),
        "the same hole drilled into the shell drains the cavity:\n{drained}"
    );
    let walled = report(&["--drain-walled"]);
    assert!(
        walled.contains("trapped resin"),
        "the cavity keeps a wall under the hole, so the resin stays in:\n{walled}"
    );
}

#[test]
fn a_hive_in_a_cavity_drains_through_one_hole() {
    // A cell with no way into the next would be a sealed box, so a hole in the lid would
    // drain the one under it and leave every other cell full.
    let path = write_box_stl("hive-drain", 20.0, 12, 0);
    let input = path.to_str().unwrap();
    let report = stdout(&estimate(&[
        input,
        "--hollow",
        "2",
        "--infill",
        "hive",
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
        "crossing holes join every cell to the next, so one hole drains all of them:\n{report}"
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

#[test]
fn an_open_floor_leaves_the_first_layer_a_ring() {
    let (out, text) = hollow_cube("open-floor", &["--hollow", "3", "--hollow-open-floor"]);

    assert_eq!(
        field(&text, "wall"),
        "3.000 mm, internal, open through the floor"
    );
    // The floor is gone under the cavity, so the layer on the plate is the same ring as
    // halfway up: 156 mm2 at a 0.1 mm pitch.
    let floor = lit_pixels(&out, 0);
    assert!(
        (floor as f32 - 15_600.0).abs() / 15_600.0 < 0.03,
        "the first layer of an open shell is a 16 mm ring round a 10 mm hole, got {floor}"
    );
}

#[test]
fn an_open_floor_is_refused_on_a_mould() {
    let path = write_box_stl("open-mould", 16.0, 12, 0);
    let output = estimate(&[
        path.to_str().unwrap(),
        "--hollow",
        "2",
        "--hollow-mode",
        "external",
        "--hollow-open-floor",
    ]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("a mould has none"));
}

#[test]
fn plugs_stand_beside_the_model_they_close() {
    let path = write_box_stl("plugs", 8.0, 12, 0);
    let out = output_dir("plugs");
    let output = slice(&[
        path.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--layer-height",
        "1",
        "--drain",
        "3",
        "--drain-at",
        "4,4,10",
        "--drain-plugs",
        "-o",
        out.to_str().unwrap(),
    ]);
    let text = stdout(&output);
    assert!(output.status.success(), "{text}");
    assert_eq!(
        field(&text, "plugs"),
        "1 beside the model, 0.15 mm narrower a side"
    );

    // The 8 mm box fills 6 400 pixels of the first layer; a plug of 1.35 mm radius standing
    // past its right-hand side adds about 570 more.
    let lit = lit_pixels(&out, 0);
    assert!(
        (lit as f32 - 6_970.0).abs() < 150.0,
        "the box and one plug beside it, about 6 970 pixels, got {lit}"
    );
}

#[test]
fn plugs_with_no_hole_are_refused() {
    let path = write_box_stl("plugs-nothing", 8.0, 12, 0);
    let output = estimate(&[path.to_str().unwrap(), "--hollow", "2", "--drain-plugs"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("none was given"));
}

#[test]
fn an_open_floor_with_no_shell_is_refused() {
    let path = write_box_stl("open-solid", 16.0, 12, 0);
    let output = estimate(&[path.to_str().unwrap(), "--hollow-open-floor"]);
    assert!(
        !output.status.success(),
        "a solid model has no floor to open"
    );
    assert!(stderr(&output).contains("--hollow was not given"));
}

#[test]
fn an_infill_with_no_shell_is_refused() {
    let path = write_box_stl("infill-solid", 16.0, 12, 0);
    for extra in [&["--infill", "hive"][..], &["--infill-cell", "6"]] {
        let mut args = vec![path.to_str().unwrap()];
        args.extend_from_slice(extra);
        let output = estimate(&args);
        assert!(!output.status.success(), "{extra:?} on a solid model");
        assert!(stderr(&output).contains(extra[0]), "the refusal names it");
    }
}

#[test]
fn a_negative_plug_gap_is_refused() {
    let path = write_box_stl("plugs-negative", 8.0, 12, 0);
    let output = estimate(&[
        path.to_str().unwrap(),
        "--drain-at",
        "4,4,10",
        "--drain-plugs",
        "--plug-gap=-0.1",
    ]);
    assert!(
        !output.status.success(),
        "a plug wider than its hole cannot go in"
    );
    assert!(stderr(&output).contains("--plug-gap"));
}

#[test]
fn plugs_that_would_run_off_the_plate_are_refused() {
    // The 40 mm plate leaves 4 mm past a 36 mm box, short of a spacing and a plug.
    let path = write_box_stl("plugs-off", 36.0, 12, 0);
    let output = estimate(&[
        path.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--drain",
        "3",
        "--drain-at",
        "18,18,40",
        "--drain-plugs",
    ]);
    assert!(!output.status.success(), "{}", stdout(&output));
    assert!(stderr(&output).contains("run off the plate"));
}

#[test]
fn a_square_hole_turned_on_its_corner_takes_a_plug() {
    let path = write_box_stl("plugs-square", 8.0, 12, 0);
    let out = output_dir("plugs-square");
    let output = slice(&[
        path.to_str().unwrap(),
        "--profile",
        test_panel().to_str().unwrap(),
        "--layer-height",
        "1",
        "--drain",
        "3",
        "--drain-sides",
        "4",
        "--drain-turn",
        "45",
        "--drain-at",
        "4,4,10",
        "--drain-plugs",
        "--plug-gap",
        "0.2",
        "-o",
        out.to_str().unwrap(),
    ]);
    let text = stdout(&output);
    assert!(output.status.success(), "{text}");
    assert_eq!(
        field(&text, "plugs"),
        "1 beside the model, 0.20 mm narrower a side"
    );
    // A square with its corners on 1.5 mm, its sides 0.2 mm in, is 2 (1.5 / root 2 - 0.2)
    // across, 3.1 mm2 or about 310 pixels over the box's 6 400.
    let lit = lit_pixels(&out, 0);
    assert!(
        (lit as f32 - 6_710.0).abs() < 100.0,
        "the box and one square plug beside it, about 6 710 pixels, got {lit}"
    );
}

/// The first layer of a 12 mm block stood on supports and a raft, with whatever follows.
fn raft_layer(name: &str, extra: &[&str]) -> Vec<u8> {
    let path = write_boxes_stl(name, &[([0.0, 0.0, 0.0], [12.0, 12.0, 4.0])]);
    let out = output_dir(name);
    let panel = test_panel();
    let mut args = vec![
        path.to_str().unwrap(),
        "--profile",
        panel.to_str().unwrap(),
        "--supports",
        "medium",
        "--layer-height",
        "0.5",
        "-o",
        out.to_str().unwrap(),
    ];
    args.extend_from_slice(extra);
    let output = slice(&args);
    assert!(output.status.success(), "{}", stderr(&output));
    layer_pixels(&out, 0).2
}

#[test]
fn a_hole_drilled_into_the_raft_opens_its_first_layer() {
    let whole = raft_layer("raft-whole", &[]);
    // Under the raft's margin past the block, where no tree stands on it.
    let drilled = raft_layer(
        "raft-drilled",
        &["--drain", "2", "--drain-support-at", "14,6,-1"],
    );
    let lit = |pixels: &[u8]| pixels.iter().filter(|pixel| **pixel > 0).count() as f32;
    // A 2 mm hole is pi 1^2 = 3.14 mm2 of the raft, 314 pixels at 0.1 mm.
    let opened = lit(&whole) - lit(&drilled);
    assert!(
        (opened - 314.0).abs() < 314.0 * 0.15,
        "the hole takes 314 px out of the raft's first layer, got {opened}"
    );
}

#[test]
fn a_hole_into_supports_with_none_asked_for_is_refused() {
    let path = write_box_stl("no-supports-to-drill", 10.0, 12, 0);
    let output = estimate(&[path.to_str().unwrap(), "--drain-support-at", "5,5,-1"]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("--drain-support-at"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn a_drain_point_off_a_corner_is_refused() {
    let path = write_box_stl("drain-corner", 16.0, 12, 0);
    let drain = |at: &str| {
        let flag = format!("--drain-at={at}");
        estimate(&[path.to_str().unwrap(), "--hollow", "2", &flag])
    };
    // The second lies in the plane of the floor, square to its normal.
    for corner in ["-20,-20,-20", "-20,-20,0"] {
        let output = drain(corner);
        assert_eq!(
            output.status.code(),
            Some(1),
            "{corner}: {}",
            stderr(&output)
        );
        assert!(
            stderr(&output).contains("edge or corner"),
            "{}",
            stderr(&output)
        );
    }
    // The last two lie on a face, where the closest point lands a few ulps off the point.
    for face in ["-20,8,8", "8,8,3", "5.3,7.7,16", "7.77,0,3.33"] {
        let output = drain(face);
        assert!(output.status.success(), "{face}: {}", stderr(&output));
    }
}
