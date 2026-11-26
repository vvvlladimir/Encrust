//! `--mirror`, `--move` and `--copies`: the window's Position moves, each an edit of the
//! project written (ADR 0478).

use core_engine::project::{Project, load};
use core_geometry::Transform;

use crate::support::*;

/// A 10 mm cube staged with `args`, and the plate the run wrote.
fn placed(name: &str, args: &[&str]) -> Project {
    let cube = write_box_stl(name, 10.0, 12, 0);
    let out = output_file(&format!("{name}.encrust"));
    let mut all = vec![
        cube.to_str().expect("a UTF-8 path"),
        "--project-out",
        out.to_str().expect("a UTF-8 path"),
    ];
    all.extend_from_slice(args);
    let output = estimate(&all);
    assert!(output.status.success(), "{}", stderr(&output));
    load(&out).expect("the project written reads back")
}

fn transforms(project: &Project) -> Vec<Transform> {
    (project.manifest.objects.iter())
        .map(|object| object.transform)
        .collect()
}

#[test]
fn copies_stand_in_a_row_beside_the_model_each_clear_of_the_last() {
    let project = placed("copied-cube", &["--copies", "2"]);
    let mut at: Vec<f32> = transforms(&project)
        .iter()
        .map(|transform| transform.translation.x)
        .collect();
    // A copy is put right after the model it copies, so the row is read in plate order.
    at.sort_by(f32::total_cmp);
    assert_eq!(at.len(), 3, "the cube and two copies");
    for pair in at.windows(2) {
        assert!(
            pair[1] - pair[0] >= 10.0,
            "each copy a 10 mm cube's width or more on, got {at:?}"
        );
    }
}

#[test]
fn a_move_shifts_the_model_by_that_much() {
    let still = transforms(&placed("unmoved-cube", &[]))[0];
    let moved = transforms(&placed("moved-cube", &["--move", "5,-2,0"]))[0];
    let by = moved.translation - still.translation;
    assert!(
        (by.x - 5.0).abs() < 1e-4 && (by.y + 2.0).abs() < 1e-4 && by.z.abs() < 1e-4,
        "moved by 5,-2,0 mm, got {by:?}"
    );
}

#[test]
fn a_mirror_turns_the_model_inside_out_across_its_axis() {
    let still = transforms(&placed("unmirrored-cube", &[]))[0];
    let mirrored = transforms(&placed("mirrored-cube", &["--mirror", "x"]))[0];
    let handedness =
        |transform: Transform| transform.scale.x * transform.scale.y * transform.scale.z;
    assert!(
        handedness(still) * handedness(mirrored) < 0.0,
        "a mirror flips the model's handedness: {still:?} then {mirrored:?}"
    );
}

#[test]
fn the_standard_machine_centres_a_model_on_the_standard_printers_plate() {
    let catalogue = printer_profiles::Catalogue::bundled().expect("the shipped catalogue reads");
    let plate = (catalogue.printer(printer_profiles::STANDARD_PRINTER))
        .expect("the standard printer ships")
        .profile
        .build_volume;
    let project = placed("standard-cube", &["--standard-machine", "--center"]);
    // The cube's file stands it from its origin, so its middle is 5 mm on from its translation.
    let at = transforms(&project)[0].translation + core_geometry::Vec3::new(5.0, 5.0, 0.0);
    let middle = (plate.x / 2.0, plate.y / 2.0);
    assert!(
        (at.x - middle.0).abs() < 0.5 && (at.y - middle.1).abs() < 0.5,
        "the cube's middle over the plate's, {middle:?} mm, got {at:?}"
    );
}

#[test]
fn a_file_is_never_written_on_the_standard_machine() {
    let cube = write_box_stl("standard-unwritten", 10.0, 12, 0);
    let out = output_file("standard-unwritten.goo");
    let output = slice(&[
        cube.to_str().expect("a UTF-8 path"),
        "--standard-machine",
        "-o",
        out.to_str().expect("a UTF-8 path"),
    ]);
    assert!(
        !output.status.success(),
        "slicing on the standard machine is refused"
    );
    assert!(!out.exists(), "nothing written");
}
