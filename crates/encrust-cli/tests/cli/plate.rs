//! A plate of several models: arranged, placed by a plate file, or opened from a
//! project the window saved.

use std::fs;
use std::path::PathBuf;
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
    let output = slice(&[
        project_fixture().to_str().unwrap(),
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

#[test]
fn a_flag_that_shapes_a_model_is_refused_for_a_project() {
    let output = estimate(&[project_fixture().to_str().unwrap(), "--hollow", "3"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("--hollow"), "{}", stderr(&output));
}

#[test]
fn blur_and_samples_per_layer_are_refused_for_a_project_too() {
    for flags in [["--blur", "2"], ["--samples-per-layer", "3"]] {
        let output = estimate(&[&[project_fixture().to_str().unwrap()][..], &flags].concat());
        assert_eq!(output.status.code(), Some(1), "{flags:?}");
        assert!(stderr(&output).contains(flags[0]), "{}", stderr(&output));
    }
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
