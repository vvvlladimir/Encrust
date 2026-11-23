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
fn the_angles_reported_are_the_angles_rotate_would_apply() {
    let turn = Vec3::new(-30.0, 0.0, 45.0);
    let summary = summary(euler_degrees(turn));
    assert_eq!(summary.rotate_degrees(), turn, "--rotate repeats the turn");
    assert!(
        summary.axis_name().is_none(),
        "two axes are not one of them"
    );
}
