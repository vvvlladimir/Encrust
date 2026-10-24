use core_geometry::Vec3;

use super::*;

/// An axis-aligned box from the origin to `size`, wound outward.
fn box_mesh(size: Vec3) -> Mesh {
    core_geometry::cuboid(Vec3::ZERO, size)
}

/// A wedge whose top face leans at 45 degrees: a right triangular prism lying along Y.
fn ramp(run: Scalar, rise: Scalar) -> Mesh {
    let vertices = vec![
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(run, 0.0, 0.0),
        Vec3::new(run, 0.0, rise),
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(run, 1.0, 0.0),
        Vec3::new(run, 1.0, rise),
    ];
    let faces = vec![
        [0, 1, 2],
        [3, 5, 4],
        [0, 2, 5],
        [0, 5, 3],
        [1, 4, 5],
        [1, 5, 2],
        [0, 3, 4],
        [0, 4, 1],
    ];
    Mesh::new(vertices, faces)
}

fn settings() -> AdaptiveSettings {
    AdaptiveSettings {
        cusp_mm: 0.05,
        min_height_mm: 0.05,
        max_height_mm: 0.20,
    }
}

#[test]
fn a_box_is_sliced_at_the_thickest_layer_allowed() {
    let plan = plan(&box_mesh(Vec3::new(2.0, 2.0, 2.0)), &settings()).expect("a box plans");

    assert_eq!(
        plan.layer_count(),
        10,
        "2 mm of vertical wall at the 0.2 mm ceiling is ten layers"
    );
    assert!(plan.is_uniform());
}

#[test]
fn a_forty_five_degree_ramp_is_sliced_thinner_than_the_ceiling() {
    // cos 45 is 0.707, so a 0.05 mm cusp allows 0.0707 mm, which is one 0.05 mm step.
    let plan = plan(&ramp(1.0, 1.0), &settings()).expect("a ramp plans");

    assert_eq!(
        plan.layer_count(),
        20,
        "1 mm of ramp at 0.05 mm is 20 layers"
    );
    for index in 0..plan.layer_count() {
        let thickness = plan.thickness_of(index).expect("a layer");
        assert!(
            (thickness - 0.05).abs() < 1e-5,
            "layer {index} is {thickness} mm, not the 0.05 mm a 45 degree face allows"
        );
    }
}

/// Two boxes stacked, the lower one ending at `shelf`: the step between them is a
/// flat face at a height the layer grid would otherwise miss.
fn stepped(shelf: Scalar) -> Mesh {
    let lower = box_mesh(Vec3::new(4.0, 4.0, shelf));
    let upper = core_geometry::transform_mesh(
        &box_mesh(Vec3::new(2.0, 2.0, 2.0 - shelf)),
        core_geometry::Transform::from_translation(Vec3::new(0.0, 0.0, shelf)),
    );
    let offset = lower.vertices.len() as u32;
    let mut vertices = lower.vertices;
    vertices.extend(upper.vertices);
    let mut faces = lower.faces;
    faces.extend(upper.faces.iter().map(|face| face.map(|i| i + offset)));
    Mesh::new(vertices, faces)
}

#[test]
fn a_tall_stack_does_not_drift_into_a_layer_above_the_model() {
    let settings = AdaptiveSettings {
        cusp_mm: 0.05,
        min_height_mm: 0.025,
        max_height_mm: 0.05,
    };
    let plan = plan(&box_mesh(Vec3::new(2.0, 2.0, 30.0)), &settings).expect("a tall box plans");

    assert_eq!(
        plan.layer_count(),
        600,
        "30 mm of vertical wall at the 0.05 mm ceiling is 600 layers and no more"
    );
    let top = plan.top_of(599).expect("the top layer");
    assert!(
        (top - 30.0).abs() < 1e-3,
        "the stack tops out at {top} mm over a model 30 mm tall"
    );
}

#[test]
fn a_shelf_gets_a_boundary_of_its_own_rather_than_the_nearest_grid_line() {
    let shelf = 0.93;
    let plan = plan(&stepped(shelf), &settings()).expect("a stepped model plans");

    let landed = (0..plan.layer_count())
        .filter_map(|index| plan.top_of(index))
        .any(|top| (top - shelf).abs() < 1e-5);
    assert!(
        landed,
        "no layer tops out at the {shelf} mm shelf, so the step prints at the wrong height"
    );
}

#[test]
fn a_shallow_ramp_is_sliced_thinner_than_a_steep_one() {
    let steep = plan(&ramp(1.0, 4.0), &settings()).expect("a steep ramp plans");
    let shallow = plan(&ramp(4.0, 4.0), &settings()).expect("a shallow ramp plans");

    assert!(
        shallow.layer_count() > steep.layer_count(),
        "the same 4 mm of rise takes {} layers shallow against {} steep",
        shallow.layer_count(),
        steep.layer_count()
    );
}

#[test]
fn every_layer_stays_inside_the_range_and_the_stack_covers_the_model() {
    let settings = AdaptiveSettings {
        cusp_mm: 0.02,
        min_height_mm: 0.03,
        max_height_mm: 0.12,
    };
    let plan = plan(&ramp(3.0, 5.0), &settings).expect("a ramp plans");

    let mut total = 0.0;
    for index in 0..plan.layer_count() {
        let thickness = plan.thickness_of(index).expect("a layer");
        assert!(
            thickness <= settings.max_height_mm + 1e-5,
            "layer {index} is {thickness} mm, over the ceiling"
        );
        total += thickness;
    }
    assert!(
        (5.0..5.0 + settings.max_height_mm).contains(&total),
        "the stack covers {total} mm of a 5 mm model: it may overshoot by under a layer"
    );
}

#[test]
fn a_ceiling_that_is_not_a_whole_number_of_the_floor_is_never_reached() {
    let settings = AdaptiveSettings {
        cusp_mm: 0.05,
        min_height_mm: 0.023,
        max_height_mm: 0.05,
    };
    assert!((settings.reachable_max_mm() - 0.046).abs() < 1e-6);

    let divisible = AdaptiveSettings {
        min_height_mm: 0.025,
        ..settings
    };
    assert!((divisible.reachable_max_mm() - 0.05).abs() < 1e-6);
}

#[test]
fn a_range_that_is_upside_down_is_rejected() {
    let broken = AdaptiveSettings {
        min_height_mm: 0.1,
        max_height_mm: 0.05,
        ..settings()
    };
    assert!(matches!(
        plan(&box_mesh(Vec3::ONE), &broken).unwrap_err(),
        SliceError::UpsideDownRange { .. }
    ));
}

#[test]
fn a_cusp_of_zero_is_rejected() {
    let broken = AdaptiveSettings {
        cusp_mm: 0.0,
        ..settings()
    };
    assert!(matches!(
        plan(&box_mesh(Vec3::ONE), &broken).unwrap_err(),
        SliceError::NonPositiveCusp(_)
    ));
}

#[test]
fn a_zero_thickness_floor_is_rejected_and_an_empty_mesh_too() {
    let broken = AdaptiveSettings {
        min_height_mm: 0.0,
        ..settings()
    };
    assert!(matches!(
        plan(&box_mesh(Vec3::ONE), &broken).unwrap_err(),
        SliceError::NonPositiveLayerHeight(_)
    ));
    assert!(matches!(
        plan(&Mesh::default(), &settings()).unwrap_err(),
        SliceError::EmptyMesh
    ));
}
