#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

#[path = "shared/bodies.rs"]
mod shared;

use std::f32::consts::PI;

use core_geometry::{Scalar, Transform, Vec3, transform_mesh};
use core_slicer::{
    AdaptiveSettings, Contour, PlaneSliceEngine, SliceEngine, SliceError, SliceSettings, Sliced,
    WINDOW_LAYERS, Winding, Windows, adaptive_plan,
};
use shared::bodies;

fn slice(mesh: &core_geometry::Mesh, layer_height: Scalar) -> Sliced {
    PlaneSliceEngine
        .slice(
            mesh,
            &SliceSettings {
                layer_height,
                ..SliceSettings::default()
            },
        )
        .expect("the body is sliceable")
}

#[test]
fn a_cube_slices_into_squares_of_its_own_cross_section() {
    let sliced = slice(&bodies::cube(10.0), 0.5);

    assert_eq!(sliced.layers.len(), 20);
    assert!(sliced.is_clean());
    for layer in &sliced.layers {
        assert_eq!(layer.contours.len(), 1, "a cube has one contour per layer");
        let contour = &layer.contours[0];
        assert_eq!(contour.winding, Winding::Outer, "material is enclosed");
        assert!(
            (contour.area() - 100.0).abs() < 1e-3,
            "a 10 mm cube has a 100 mm^2 cross-section, got {}",
            contour.area()
        );
    }
}

#[test]
fn a_cube_exactly_as_tall_as_a_whole_number_of_layers_keeps_every_layer() {
    // Every vertex of this cube sits exactly on a layer boundary, the case a naive
    // slicer loses or duplicates.
    let sliced = slice(&bodies::cube(1.0), 0.1);

    assert_eq!(sliced.layers.len(), 10);
    assert!(sliced.is_clean());
    assert!(sliced.layers.iter().all(|layer| layer.contours.len() == 1));
}

#[test]
fn a_sphere_slices_into_circles_of_the_radius_pythagoras_predicts() {
    let radius = 10.0;
    let sliced = slice(&bodies::uv_sphere(radius, 128, 128), 1.0);
    let centre = sliced
        .layers
        .iter()
        .find(|layer| layer.z > 0.0)
        .expect("a sphere reaches above its equator");

    assert_eq!(centre.contours.len(), 1);
    // The body is centred on the origin, so the plane height is also its offset from
    // the equator.
    let z = centre.z;
    let expected = (radius * radius - z * z).sqrt();
    let found = (centre.contours[0].area() / PI).sqrt();
    assert!(
        (found - expected).abs() < 0.05,
        "a plane {z:.3} mm off centre cuts a circle of radius {expected:.3} mm, got {found:.3}"
    );
}

#[test]
fn a_sphere_is_clean_at_every_height() {
    let sliced = slice(&bodies::uv_sphere(10.0, 64, 64), 0.25);

    assert!(sliced.is_clean());
    assert!(sliced.layers.iter().all(|layer| !layer.contours.is_empty()));
}

#[test]
fn a_tetrahedron_cross_section_shrinks_the_way_its_slope_says() {
    let sliced = slice(&bodies::tetrahedron(), 0.1);

    for layer in &sliced.layers {
        assert_eq!(layer.contours.len(), 1);
        // The corner tetrahedron's section at height z is a right triangle with legs
        // 1 - z, so its area is (1 - z)^2 / 2.
        let expected = (1.0 - layer.z).powi(2) / 2.0;
        let found = layer.contours[0].area();
        assert!(
            (found - expected).abs() < 1e-4,
            "at z = {:.3} the section area is (1 - z)^2 / 2 = {expected:.5}, got {found:.5}",
            layer.z
        );
    }
}

#[test]
fn a_hole_through_a_cube_is_wound_the_other_way() {
    let sliced = slice(&cube_with_a_shaft(), 1.0);
    let layer = &sliced.layers[5];

    assert_eq!(layer.contours.len(), 2);
    let outer = contour_with(layer, Winding::Outer);
    let inner = contour_with(layer, Winding::Inner);
    assert!((outer.area() - 100.0).abs() < 1e-3);
    assert!((inner.area() - 4.0).abs() < 1e-3);
}

#[test]
fn a_missing_lid_changes_nothing_because_it_never_crosses_a_plane() {
    let sliced = slice(&bodies::open_box(10.0), 1.0);

    assert!(
        sliced.is_clean(),
        "a horizontal face contributes no crossing"
    );
    assert!(sliced.layers.iter().all(|layer| layer.contours.len() == 1));
}

#[test]
fn a_missing_wall_is_closed_over_and_reported() {
    let sliced = slice(&cube_missing_a_wall(), 1.0);

    assert!(!sliced.is_clean());
    assert!(
        sliced.open_contours > 0,
        "a hole in a wall leaves a chain that cannot close on topology alone"
    );
}

/// A cube with the two triangles of its y = 0 wall removed, so every layer meets a gap.
fn cube_missing_a_wall() -> core_geometry::Mesh {
    let mut mesh = bodies::cube(10.0);
    mesh.faces.drain(4..6);
    mesh
}

#[test]
fn slicing_does_not_depend_on_where_the_model_sits_in_z() {
    let mesh = bodies::cube(4.0);
    let lifted = transform_mesh(
        &mesh,
        Transform::from_translation(core_geometry::Vec3::new(0.0, 0.0, 37.5)),
    );

    let low = slice(&mesh, 0.25);
    let high = slice(&lifted, 0.25);

    assert_eq!(low.layers.len(), high.layers.len());
    for (a, b) in low.layers.iter().zip(&high.layers) {
        assert!((a.contours[0].area() - b.contours[0].area()).abs() < 1e-3);
    }
}

/// A 10 mm cube with a 2 mm square shaft through it, so every layer has a hole.
fn cube_with_a_shaft() -> core_geometry::Mesh {
    use core_geometry::{Mesh, Vec3};

    let (outer, inner) = (10.0, 2.0);
    let offset = (outer - inner) / 2.0;
    let mut vertices = Vec::new();
    let mut faces = Vec::new();

    // Eight corners of the outer box, then eight of the shaft, bottom ring first.
    for (size, base) in [(outer, 0.0), (inner, offset)] {
        for z in [0.0, outer] {
            for (x, y) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
                vertices.push(Vec3::new(base + x * size, base + y * size, z));
            }
        }
    }

    // Outer walls wound counter-clockwise seen from outside, shaft walls the other way
    // so the solid stays on the same side of both.
    for (base, flip) in [(0u32, false), (8, true)] {
        for corner in 0..4u32 {
            let next = (corner + 1) % 4;
            let (a, b, c, d) = (
                base + corner,
                base + next,
                base + 4 + next,
                base + 4 + corner,
            );
            if flip {
                faces.push([a, c, b]);
                faces.push([a, d, c]);
            } else {
                faces.push([a, b, c]);
                faces.push([a, c, d]);
            }
        }
    }
    Mesh::new(vertices, faces)
}

fn contour_with(layer: &core_slicer::Layer, winding: Winding) -> &Contour {
    layer
        .contours
        .iter()
        .find(|contour| contour.winding == winding)
        .expect("the layer carries both windings")
}

/// A sphere is vertical at its equator and lies flat at its poles, so an adaptive plan
/// has to reach the ceiling round the middle and the floor at both ends. See ADR 0091.
#[test]
fn a_sphere_is_planned_thick_at_its_equator_and_thin_at_its_poles() {
    let radius = 10.0;
    let settings = AdaptiveSettings {
        cusp_mm: 0.03,
        min_height_mm: 0.02,
        max_height_mm: 0.10,
    };
    let standing = transform_mesh(
        &bodies::uv_sphere(radius, 128, 128),
        Transform::from_translation(Vec3::new(0.0, 0.0, radius)),
    );
    let plan = adaptive_plan(&standing, &settings).expect("a sphere plans");

    // The sphere stands on the plate, so the equator is the middle of the stack.
    let thickness_at = |z: Scalar| {
        (0..plan.layer_count())
            .find(|&index| plan.top_of(index).is_some_and(|top| top >= z))
            .and_then(|index| plan.thickness_of(index))
            .expect("a layer at that height")
    };

    let equator = thickness_at(radius);
    let pole = thickness_at(2.0 * radius - 0.05);
    assert!(
        (equator - settings.max_height_mm).abs() < 1e-5,
        "the equator is a vertical wall and takes the ceiling, not {equator} mm"
    );
    assert!(
        (pole - settings.min_height_mm).abs() < 1e-5,
        "the pole lies flat and takes the floor, not {pole} mm"
    );
    assert!(!plan.is_uniform());
}

/// A 10 mm cube sunk 4 mm into the plate: only the 6 mm standing on it are cut, from the
/// plate up, so no layer drives the plate into the vat floor.
#[test]
fn what_stands_under_the_plate_is_not_cut() {
    let sunk = transform_mesh(
        &bodies::cube(10.0),
        Transform::from_translation(Vec3::new(0.0, 0.0, -4.0)),
    );

    let stack = slice(&sunk, 0.5);
    assert_eq!(
        stack.layers.len(),
        12,
        "6 mm over the plate at 0.5 mm a layer"
    );
    assert!(
        stack.layers.iter().all(|layer| layer.z > 0.0),
        "every plane is over the plate"
    );

    let settings = AdaptiveSettings {
        cusp_mm: 0.03,
        min_height_mm: 0.02,
        max_height_mm: 0.10,
    };
    let plan = adaptive_plan(&sunk, &settings).expect("the cube stands partly on the plate");
    assert!(
        plan.band_of(0).is_some_and(|(bottom, _)| bottom == 0.0),
        "an adaptive stack starts on the plate too"
    );
}

#[test]
fn a_model_wholly_under_the_plate_is_refused() {
    let buried = transform_mesh(
        &bodies::cube(10.0),
        Transform::from_translation(Vec3::new(0.0, 0.0, -12.0)),
    );
    let settings = SliceSettings {
        layer_height: 0.5,
        ..SliceSettings::default()
    };

    assert!(matches!(
        PlaneSliceEngine.slice(&buried, &settings),
        Err(SliceError::UnderThePlate { top_mm }) if (top_mm + 2.0).abs() < 1e-5
    ));
    assert!(matches!(
        Windows::new(&buried, settings, WINDOW_LAYERS),
        Err(SliceError::UnderThePlate { .. })
    ));
    let adaptive = AdaptiveSettings {
        cusp_mm: 0.03,
        min_height_mm: 0.02,
        max_height_mm: 0.10,
    };
    assert!(matches!(
        adaptive_plan(&buried, &adaptive),
        Err(SliceError::UnderThePlate { .. })
    ));
}
