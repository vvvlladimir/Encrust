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
