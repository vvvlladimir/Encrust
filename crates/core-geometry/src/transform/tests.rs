use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};

use super::*;

#[test]
fn lifting_stands_a_body_clear_of_the_plate() {
    let bounds = Aabb::new(Vec3::new(-1.0, -1.0, 3.0), Vec3::new(1.0, 1.0, 9.0));
    let offset = lift_over_plate(&bounds, 5.0);
    assert!(
        (bounds.mins.z + offset.z - 5.0).abs() < 1e-6,
        "the lowest point ends up at the lift, got {}",
        bounds.mins.z + offset.z
    );
    assert!(
        offset.x.abs() < 1e-6 && offset.y.abs() < 1e-6,
        "only z moves"
    );
}

#[test]
fn lifting_by_nothing_is_dropping_to_the_plate() {
    let bounds = Aabb::new(Vec3::new(0.0, 0.0, -2.0), Vec3::new(1.0, 1.0, 4.0));
    assert_eq!(lift_over_plate(&bounds, 0.0), drop_to_plate(&bounds));
}

#[test]
fn default_transform_is_identity() {
    assert_eq!(Transform::default().to_matrix(), Mat4::IDENTITY);
}

#[test]
fn mirroring_scale_reverses_winding() {
    let mesh = Mesh::new(vec![Vec3::ZERO, Vec3::X, Vec3::Y], vec![[0, 1, 2]]);
    let mirror = Transform {
        scale: Vec3::new(-1.0, 1.0, 1.0),
        ..Transform::default()
    };
    assert_eq!(transform_mesh(&mesh, mirror).faces, vec![[0, 2, 1]]);
    assert_eq!(
        transform_mesh(&mesh, Transform::default()).faces,
        vec![[0, 1, 2]]
    );
}

#[test]
fn placement_helpers_move_the_bounds_where_asked() {
    let bounds = Aabb::new(Vec3::new(2.0, 2.0, 5.0), Vec3::new(4.0, 6.0, 9.0));
    assert_eq!(drop_to_plate(&bounds), Vec3::new(0.0, 0.0, -5.0));
    assert_eq!(
        center_over_plate(&bounds, 100.0, 60.0),
        Vec3::new(47.0, 26.0, 0.0)
    );
}

#[test]
fn scale_then_rotate_then_translate() {
    let t = Transform::from_scale_rotation_translation(
        Vec3::splat(2.0),
        Quat::from_rotation_z(FRAC_PI_2),
        Vec3::new(0.0, 0.0, 5.0),
    );
    let moved = t.to_matrix().transform_point3(Vec3::X);
    assert!(moved.abs_diff_eq(Vec3::new(0.0, 2.0, 5.0), 1e-5));
}

fn turned_on_its_edge() -> Transform {
    Transform::from_scale_rotation_translation(
        Vec3::new(1.0, 2.0, 3.0),
        Quat::from_rotation_x(FRAC_PI_4),
        Vec3::new(5.0, 6.0, 7.0),
    )
}

#[test]
fn a_stretch_along_the_plate_multiplies_that_axis_alone() {
    let placed = turned_on_its_edge();
    let stretched = placed.stretched(Vec3::new(1.0, 1.0, 2.0));
    let (before, after) = (placed.linear(), stretched.linear());
    for column in 0..3 {
        let (from, to) = (before.col(column), after.col(column));
        assert!(
            to.abs_diff_eq(from * Vec3::new(1.0, 1.0, 2.0), 1e-5),
            "every model axis grows only in Z, column {column}: {from:?} to {to:?}"
        );
    }
    assert!(
        stretched
            .to_matrix()
            .transform_point3(Vec3::ZERO)
            .abs_diff_eq(placed.translation, 1e-6),
        "the stretch is about the model's origin, not the plate's"
    );
}

#[test]
fn a_turn_carries_a_stretch_with_it() {
    let stretched = turned_on_its_edge().stretched(Vec3::new(1.0, 1.0, 2.0));
    let turn = Quat::from_rotation_y(0.7);
    let expected = Mat3::from_quat(turn) * stretched.linear();
    assert!(
        stretched.turned(turn).linear().abs_diff_eq(expected, 1e-5),
        "turning a stretched model turns the stretched shape as it stands"
    );
}

#[test]
fn the_plate_factors_read_back_a_stretch() {
    let placed = Transform::from_scale_rotation_translation(
        Vec3::splat(2.0),
        Quat::from_rotation_z(0.4),
        Vec3::ZERO,
    );
    assert!(placed.plate_factors().abs_diff_eq(Vec3::splat(2.0), 1e-5));
    let stretched = placed.stretched(Vec3::new(1.5, 1.0, 1.0));
    assert!(
        stretched
            .plate_factors()
            .abs_diff_eq(Vec3::new(3.0, 2.0, 2.0), 1e-5),
        "X read 2 and was stretched by 1.5, got {:?}",
        stretched.plate_factors()
    );
}

#[test]
fn unturned_is_the_scale_for_a_model_only_scaled() {
    let placed = Transform::from_scale_rotation_translation(
        Vec3::new(-1.5, 2.0, 0.5),
        Quat::from_rotation_y(1.1),
        Vec3::ONE,
    );
    assert!(
        placed
            .unturned()
            .abs_diff_eq(Mat3::from_diagonal(Vec3::new(1.5, 2.0, 0.5)), 1e-5),
        "a turn and a mirror measure nothing, got {:?}",
        placed.unturned()
    );
}

#[test]
fn a_model_only_turned_is_unturned_to_exactly_the_identity() {
    let turned = Transform::default().turned(Quat::from_euler(glam::EulerRot::XYZ, 0.7, 1.3, -2.1));
    assert_eq!(
        turned.unturned(),
        Mat3::IDENTITY,
        "the hollow's fast path and the project's keys test for it exactly"
    );
}

#[test]
fn unturned_measures_as_the_plate_does_and_ignores_a_turn() {
    let stretched = turned_on_its_edge().stretched(Vec3::new(2.0, 0.5, 1.5));
    let unturned = stretched.unturned();
    let metric = stretched.linear().transpose() * stretched.linear();
    assert!((unturned.transpose() * unturned).abs_diff_eq(metric, 1e-4));
    let turned = stretched.turned(Quat::from_rotation_z(2.0)).unturned();
    assert!(
        turned.abs_diff_eq(unturned, 1e-4),
        "{turned:?} against {unturned:?}"
    );
}

#[test]
fn the_least_scale_is_the_shortest_axis_of_the_stretched_body() {
    let placed = Transform::from_scale_rotation_translation(
        Vec3::new(2.0, 3.0, 0.5),
        Quat::from_rotation_z(0.3),
        Vec3::ZERO,
    );
    assert!((placed.least_scale() - 0.5).abs() < 1e-5);
    // An unscaled model turned anyhow and squashed to a quarter along the plate's Z.
    let squashed = Transform::from_scale_rotation_translation(
        Vec3::ONE,
        Quat::from_rotation_x(0.9),
        Vec3::ZERO,
    )
    .stretched(Vec3::new(1.0, 1.0, 0.25));
    assert!(
        (squashed.least_scale() - 0.25).abs() < 1e-4,
        "the plate's Z is squashed to a quarter, got {}",
        squashed.least_scale()
    );
}

#[test]
fn a_stretch_does_not_mirror_and_a_negative_scale_does() {
    let stretched = turned_on_its_edge().stretched(Vec3::new(3.0, 1.0, 1.0));
    assert!(!stretched.is_mirrored());
    let mirrored = Transform {
        scale: Vec3::new(-1.0, 1.0, 1.0),
        ..stretched
    };
    assert!(mirrored.is_mirrored());
}

#[test]
fn the_turn_put_back_on_the_sizing_is_the_placement() {
    let placed = turned_on_its_edge().stretched(Vec3::new(0.5, 2.0, 1.5));
    let sizing = placed.sizing();
    let rebuilt = Mat3::from_quat(placed.rotation) * sizing.linear();
    assert!(rebuilt.abs_diff_eq(placed.linear(), 1e-5));
    assert_eq!(
        Transform::from_scale_rotation_translation(
            Vec3::splat(2.0),
            Quat::from_rotation_z(1.0),
            Vec3::ONE
        )
        .sizing(),
        Transform {
            scale: Vec3::splat(2.0),
            ..Transform::default()
        },
        "a model only scaled bakes its scale alone"
    );
}

#[test]
fn a_stretch_along_one_of_the_models_own_axes_stays_a_scale() {
    let lying = Transform::from_scale_rotation_translation(
        Vec3::new(1.0, 2.0, 3.0),
        Quat::from_rotation_x(FRAC_PI_2),
        Vec3::ZERO,
    );
    let stretched = lying.stretched(Vec3::new(1.0, 1.0, 2.0));
    assert_eq!(stretched.stretch, Mat3::IDENTITY);
    assert!(
        stretched.scale.abs_diff_eq(Vec3::new(1.0, 4.0, 3.0), 1e-6),
        "laid on its back, the model's Y stands along the plate's Z, got {:?}",
        stretched.scale
    );
    let slanted = turned_on_its_edge().stretched(Vec3::new(1.0, 1.0, 2.0));
    assert_ne!(
        slanted.stretch,
        Mat3::IDENTITY,
        "at 45 degrees no axis of the model is Z"
    );
}
