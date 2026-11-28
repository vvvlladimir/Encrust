use super::oriented::OrientSummary;
use core_document::euler_degrees;
use core_geometry::{Quat, Vec3};

fn summary(rotation: Quat) -> OrientSummary {
    OrientSummary {
        rotation,
        peak_mm2: 0.0,
        overhang_mm2: 0.0,
        on_plate: false,
        choices: Vec::new(),
    }
}

#[test]
fn a_turn_about_one_axis_names_it_and_the_flag_that_repeats_it() {
    let quarter = summary(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2));
    assert_eq!(
        quarter.describe(),
        "turned 90 degrees around X (--rotate 90,0,0)"
    );
}

#[test]
fn the_flag_repeats_a_turn_to_a_hundredth_of_a_degree() {
    let tilted = summary(euler_degrees(Vec3::new(30.27, 0.0, 0.0)));
    assert!(
        tilted.describe().ends_with("(--rotate 30.27,0,0)"),
        "a whole degree would tilt a flat face: {}",
        tilted.describe()
    );
}

#[test]
fn the_angles_reported_are_the_angles_rotate_would_apply() {
    let turn = Vec3::new(-30.0, 0.0, 45.0);
    let summary = summary(euler_degrees(turn));
    let read = summary.rotate_degrees();
    assert!(
        read.abs_diff_eq(turn, 1e-3),
        "--rotate repeats the turn, got {read}"
    );
    assert!(
        summary.axis_name().is_none(),
        "two axes are not one of them"
    );
}
