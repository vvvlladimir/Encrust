//! A plate of several models: arranged, placed by a plate file, or opened from a
//! project the window saved.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

use crate::support::*;

/// What a run cured, in cubic millimetres, out of `estimate --json`.
fn cured_mm3(output: &Output) -> f64 {
    assert!(output.status.success(), "{}", stderr(output));
    json(output)["print"]["resin_mm3"]
        .as_f64()
        .unwrap_or_else(|| panic!("no print.resin_mm3 in {}", stdout(output)))
}

#[test]
fn two_models_arranged_stand_apart_and_cure_twice_one() {
    let a = write_box_stl("arrange-a", 10.0, 12, 0);
    let b = write_box_stl("arrange-b", 10.0, 12, 0);
    let (a, b) = (a.to_str().unwrap(), b.to_str().unwrap());
    let profile = shipped_profile();
    let run = |extra: &[&str]| {
        estimate(
            &[
                &[
                    a,
                    b,
                    "--profile",
                    profile.to_str().unwrap(),
                    "--layer-height",
                    "5",
                ][..],
                extra,
                &["--json"],
            ]
            .concat(),
        )
    };

    // Two 10 mm cubes are 2000 mm3; left where their files put them they are the same
    // cube twice, which cures 1000. Anti-aliased edges cost a fraction of a pixel row.
    let apart = cured_mm3(&run(&["--arrange"]));
    let stacked = cured_mm3(&run(&[]));
    assert!((apart - 2000.0).abs() < 40.0, "arranged: {apart}");
    assert!(
        (stacked - 1000.0).abs() < 20.0,
        "on top of each other: {stacked}"
    );
    assert_eq!(
        json(&run(&["--arrange"]))["models"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn center_is_refused_for_several_models() {
    let a = write_box_stl("center-a", 10.0, 12, 0);
    let b = write_box_stl("center-b", 10.0, 12, 0);
    let output = estimate(&[a.to_str().unwrap(), b.to_str().unwrap(), "--center"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("--arrange"), "{}", stderr(&output));
}

/// A plate file beside two cubes, standing where `positions` say.
fn plate_file(name: &str, body: &str) -> PathBuf {
    let cube = write_box_stl(&format!("{name}-cube"), 10.0, 12, 0);
    let dir = cube.parent().expect("the cube is in a directory");
    let path = dir.join(format!("{name}.toml"));
    let cube_name = cube.file_name().and_then(|name| name.to_str()).unwrap();
    fs::write(&path, body.replace("CUBE", cube_name)).expect("write the plate file");
    path
}

#[test]
fn a_plate_file_places_each_model_where_it_says() {
    let plate = plate_file(
        "two-placed",
        r#"
        layer_height_mm = 5
        [[model]]
        path = "CUBE"
        position = [30, 30]
        [[model]]
        path = "CUBE"
        position = [60, 30]
        scale = [1, 1, 2]
        "#,
    );
    let output = estimate(&[
        plate.to_str().unwrap(),
        "--profile",
        shipped_profile().to_str().unwrap(),
        "--json",
    ]);

    // One cube and one stretched to twice its height: 1000 + 2000 mm3.
    let cured = cured_mm3(&output);
    assert!((cured - 3000.0).abs() < 60.0, "cured {cured}");
    assert_eq!(
        json(&output)["slicing"]["layer_height_mm"].as_f64(),
        Some(5.0)
    );
}

#[test]
fn a_plate_names_the_model_each_cavity_belongs_to() {
    let plate = plate_file(
        "named-hollow",
        r#"
        layer_height_mm = 5
        [[model]]
        path = "CUBE"
        position = [30, 30]
        [[model]]
        path = "CUBE"
        position = [60, 30]
        hollow = { wall_mm = 1.0 }
        "#,
    );
    let text = stdout(&estimate(&[
        plate.to_str().unwrap(),
        "--profile",
        shipped_profile().to_str().unwrap(),
        "--precision",
        "0.1",
    ]));

    let named = text
        .lines()
        .position(|line| line.ends_with("named-hollow-cube.stl"))
        .map(|first| {
            text.lines()
                .skip(first + 1)
                .position(|line| line.trim_start().starts_with("wall "))
        });
    assert!(
        matches!(named, Some(Some(_))),
        "the cavity stands under the model it was cut into:\n{text}"
    );
}

#[test]
fn a_flag_wins_over_the_plate_file() {
    let plate = plate_file(
        "flag-wins",
        "layer_height_mm = 5\n[[model]]\npath = \"CUBE\"\nposition = [30, 30]\n",
    );
    let output = estimate(&[
        plate.to_str().unwrap(),
        "--profile",
        shipped_profile().to_str().unwrap(),
        "--layer-height",
        "2.5",
        "--json",
    ]);
    assert_eq!(
        json(&output)["slicing"]["layer_height_mm"].as_f64(),
        Some(2.5)
    );
}

#[test]
fn a_cavity_the_plate_file_hollows_is_held_up_when_asked() {
    let plate = plate_file(
        "cavity-held",
        r#"
        layer_height_mm = 0.05
        [[model]]
        path = "CUBE"
        position = [30, 30]
        hollow = { wall_mm = 1.0 }
        supports = "medium"
        "#,
    );
    let contacts = |more: &[&str]| {
        let output = estimate(&[&[plate.to_str().unwrap(), "--json"][..], more].concat());
        assert!(output.status.success(), "{}", stderr(&output));
        json(&output)["models"][0]["supports"]["contacts"]
            .as_u64()
            .unwrap_or(0)
    };
    let outside = contacts(&[]);
    let inside = contacts(&["--support-cavity"]);
    assert!(
        inside > outside,
        "the 8 mm cavity has a flat ceiling to hold: {inside} contacts against {outside}"
    );
}

#[test]
fn a_plate_both_arranged_and_placed_is_refused() {
    let plate = plate_file(
        "arranged-and-placed",
        "arrange = true\n[[model]]\npath = \"CUBE\"\nposition = [30, 30]\n",
    );
    let output = estimate(&[plate.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("position"), "{}", stderr(&output));
}

fn project_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cube.encrust")
}

#[test]
fn a_project_slices_as_the_window_saved_it() {
    let out = output_file("project.goo");
    let panel = project_fixture().with_file_name("project-panel.toml");
    let resin = test_resin();
    let output = slice(&[
        project_fixture().to_str().unwrap(),
        "--profile",
        panel.to_str().unwrap(),
        "--material",
        resin.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--json",
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(out.exists());

    // The fixture is a 20 mm cube hollowed to a 2 mm wall, on one support: the wall is
    // 8000 - 4096 mm3, and the support adds a little.
    let report = json(&output);
    let cured = report["cured"]["resin_mm3"]
        .as_f64()
        .expect("the file was written");
    assert!((3904.0..4300.0).contains(&cured), "cured {cured}");
    assert_eq!(report["models"][0]["input"].as_str(), Some("cube"));
}

/// The fixture was saved before a project named its printer by id alone (ADR 0261), and
/// carried one with none: there is nothing here to slice it on.
#[test]
fn a_project_naming_no_printer_asks_for_one() {
    let output = estimate(&[project_fixture().to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("--printer"), "{}", stderr(&output));
}

#[test]
fn a_flag_that_shapes_a_model_is_refused_for_a_project() {
    let output = estimate(&[project_fixture().to_str().unwrap(), "--hollow", "3"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("--hollow"), "{}", stderr(&output));
}

#[test]
fn infill_is_refused_for_a_project_too() {
    let output = estimate(&[project_fixture().to_str().unwrap(), "--infill", "hive"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("--infill"), "{}", stderr(&output));
}

#[test]
fn a_project_takes_the_print_settings_the_flags_set() {
    let out = output_file("project-flags.goo");
    let output = slice_project(
        &project_fixture(),
        &[
            "--no-anti-alias",
            "--samples-per-layer",
            "3",
            "-o",
            out.to_str().unwrap(),
        ],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    let text = stdout(&output);
    assert_eq!(
        field(&text, "shading"),
        "binary, no intermediate grey",
        "{text}"
    );
}

#[test]
fn a_project_is_sliced_on_its_own() {
    let cube = write_box_stl("beside-project", 10.0, 12, 0);
    let output = estimate(&[project_fixture().to_str().unwrap(), cube.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("on its own"),
        "{}",
        stderr(&output)
    );
}

/// The cube fixture with a second plate, "Small parts", holding a copy of the cube.
fn two_plate_project(name: &str) -> PathBuf {
    let load = || core_engine::project::load(&project_fixture()).expect("a fixture");
    let mut project = load();
    let mut copy = load().manifest.objects.remove(0);
    copy.plate = 1;
    copy.name = "copy".to_owned();
    project.manifest.objects.push(copy);
    project.models.push(project.models[0].clone());
    project.manifest.plates.push("Small parts".to_owned());
    let path = output_file(&format!("{name}.encrust"));
    core_engine::project::save(&path, &project).expect("the temp dir is writable");
    path
}

fn slice_project(project: &Path, extra: &[&str]) -> Output {
    let panel = project_fixture().with_file_name("project-panel.toml");
    let resin = test_resin();
    let mut args = vec![
        project.to_str().unwrap(),
        "--profile",
        panel.to_str().unwrap(),
        "--material",
        resin.to_str().unwrap(),
    ];
    args.extend_from_slice(extra);
    slice(&args)
}

#[test]
fn a_project_of_several_plates_says_which_it_slices() {
    let project = two_plate_project("plates-warned");
    let out = output_file("plates-warned.goo");
    let output = slice_project(&project, &["-o", out.to_str().unwrap()]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("--all-plates"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn plate_picks_one_plate_of_a_project() {
    let project = two_plate_project("plates-picked");
    let out = output_file("plates-picked.goo");
    let output = slice_project(
        &project,
        &["--plate", "2", "-o", out.to_str().unwrap(), "--json"],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(json(&output)["models"][0]["input"].as_str(), Some("copy"));
}

#[test]
fn a_plate_the_project_lacks_is_refused() {
    let project = two_plate_project("plates-missing");
    let out = output_file("plates-missing.goo");
    let output = slice_project(&project, &["--plate", "3", "-o", out.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("has 2"), "{}", stderr(&output));
}

#[test]
fn all_plates_writes_each_plate_beside_the_output() {
    let project = two_plate_project("plates-all");
    let out = output_file("plates-all.goo");
    let output = slice_project(
        &project,
        &["--all-plates", "-o", out.to_str().unwrap(), "--json"],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    for plate in ["plate-1", "small-parts"] {
        let written = out.with_file_name(format!("encrust-cli-out-plates-all-{plate}.goo"));
        assert!(written.exists(), "{} was written", written.display());
    }
    let plates = json(&output)["plates"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert_eq!(plates.len(), 2, "one report a plate");
}

#[test]
fn plate_and_all_plates_are_for_a_project_only() {
    let cube = write_box_stl("plate-of-a-model", 10.0, 12, 0);
    let output = estimate(&[cube.to_str().unwrap(), "--plate", "1"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("project"), "{}", stderr(&output));
    let output = slice(&[cube.to_str().unwrap(), "--all-plates"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("project"), "{}", stderr(&output));
}
