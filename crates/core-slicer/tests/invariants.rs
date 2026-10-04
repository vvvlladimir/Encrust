#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

#[path = "shared/bodies.rs"]
mod shared;

use core_geometry::{Mesh, Quat, Scalar, Transform, Vec3, lift_over_plate, transform_mesh};
use core_slicer::{PlaneSliceEngine, SliceEngine, SliceSettings, Winding};
use proptest::prelude::*;
use shared::bodies;

fn sliced(mesh: &Mesh, layer_height: Scalar) -> core_slicer::Sliced {
    PlaneSliceEngine
        .slice(
            mesh,
            &SliceSettings {
                layer_height,
                ..SliceSettings::default()
            },
        )
        .expect("a non-empty mesh with a positive layer height is sliceable")
}

proptest! {
    /// Closed in, closed out: however a watertight body is placed, every chain of
    /// crossings has to come back to where it started.
    #[test]
    fn a_closed_body_yields_closed_contours(
        angles in prop::array::uniform3(0.0..std::f32::consts::TAU),
        scale in 0.5..4.0f32,
        lift in 0.0..20.0f32,
        layer_height in 0.05..1.0f32,
    ) {
        let rotation = Quat::from_rotation_z(angles[2])
            * Quat::from_rotation_y(angles[1])
            * Quat::from_rotation_x(angles[0]);
        let turned = transform_mesh(
            &bodies::cube(10.0),
            Transform {
                rotation,
                scale: Vec3::splat(scale),
                ..Transform::default()
            },
        );
        // Over the plate, since nothing under it is cut.
        let bounds = turned.aabb().expect("a cube has bounds");
        let placed = transform_mesh(&turned, Transform::from_translation(lift_over_plate(&bounds, lift)));

        prop_assert!(sliced(&placed, layer_height).is_clean());
    }

    /// The stack has to cover the model and no more.
    #[test]
    fn the_layer_count_follows_the_height(
        size in 1.0..50.0f32,
        layer_height in 0.02..2.0f32,
    ) {
        let stack = sliced(&bodies::cube(size), layer_height);
        let expected = (size / layer_height).ceil() as usize;

        prop_assert!(stack.layers.len().abs_diff(expected) <= 1);
        prop_assert!(stack.layers.iter().all(|layer| layer.z > 0.0 && layer.z < size));
    }

    /// Cavalieri: a cube's sections are constant, so the midpoint rule is exact and the
    /// layers must add back up to the volume.
    #[test]
    fn the_layers_of_a_cube_add_up_to_its_volume(
        size in 1.0..20.0f32,
        layers in 1usize..64,
    ) {
        let layer_height = size / layers as Scalar;
        let stack = sliced(&bodies::cube(size), layer_height);
        let volume: Scalar = stack
            .layers
            .iter()
            .flat_map(|layer| &layer.contours)
            .map(|contour| contour.area() * layer_height)
            .sum();

        prop_assert!((volume - size.powi(3)).abs() < size.powi(3) * 1e-3);
    }

    /// A solid without holes never produces a clockwise contour.
    #[test]
    fn a_solid_body_encloses_material_everywhere(layer_height in 0.1..2.0f32) {
        let stack = sliced(&bodies::uv_sphere(10.0, 48, 48), layer_height);

        prop_assert!(stack.layers.iter()
            .flat_map(|layer| &layer.contours)
            .all(|contour| contour.winding == Winding::Outer));
    }
}
