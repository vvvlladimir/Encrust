use super::*;
use std::sync::Arc;

/// Axis-aligned cube spanning 0..1 on every axis, twelve triangles.
fn unit_cube() -> Mesh {
    core_geometry::cuboid(Vec3::ZERO, Vec3::ONE)
}

#[test]
fn two_picks_make_a_span_and_the_third_starts_again() {
    let mut measure = Measure::default();
    assert!(measure.span().is_none());

    measure.pick(Vec3::ZERO, Snapped::Free);
    assert!(measure.span().is_none(), "one end is not a measurement");

    measure.pick(Vec3::new(3.0, 4.0, 0.0), Snapped::Free);
    assert_eq!(measure.distance_mm(), Some(5.0), "a 3-4-5 triangle");

    measure.pick(Vec3::new(1.0, 1.0, 1.0), Snapped::Free);
    assert!(measure.span().is_none());
    assert_eq!(measure.start(), Some(Vec3::new(1.0, 1.0, 1.0)));
}

#[test]
fn the_span_is_reported_axis_by_axis() {
    let mut measure = Measure::default();
    measure.pick(Vec3::new(1.0, 2.0, 3.0), Snapped::Free);
    measure.pick(Vec3::new(4.0, 2.0, 8.0), Snapped::Free);
    assert_eq!(measure.delta_mm(), Some(Vec3::new(3.0, 0.0, 5.0)));
}

#[test]
fn clearing_forgets_both_ends_and_keeps_the_settings() {
    let mut measure = Measure {
        snap: Snap::Edge,
        show_axes: true,
        ..Measure::default()
    };
    measure.pick(Vec3::ZERO, Snapped::Free);
    measure.pick(Vec3::X, Snapped::Free);
    measure.clear();
    assert!(measure.span().is_none() && measure.start().is_none());
    assert_eq!((measure.snap, measure.show_axes), (Snap::Edge, true));
}

#[test]
fn alt_measures_freely_whatever_is_chosen() {
    let measure = Measure {
        snap: Snap::Corner,
        ..Measure::default()
    };
    assert_eq!(measure.snap_now(true), Snap::Free);
    assert_eq!(
        measure.snap_now(false),
        Snap::Corner,
        "let go, the choice is back"
    );
}

#[test]
fn a_corner_snap_passes_over_an_edge_for_its_corner() {
    let point = Vec3::new(0.4, 0.1, 1.0);
    let (snapped, by) = snap_on_cube(Transform::default(), point, Snap::Corner);
    assert_eq!(by, Snapped::Corner);
    assert!(
        snapped.abs_diff_eq(Vec3::new(0.0, 0.0, 1.0), 1e-5),
        "the nearest corner of the top, got {snapped}"
    );
}

#[test]
fn an_edge_snap_passes_over_a_corner_for_its_edge() {
    let (snapped, by) = snap_on_cube(Transform::default(), Vec3::new(0.95, 0.9, 1.0), Snap::Edge);
    assert_eq!(by, Snapped::Edge);
    assert!(
        snapped.abs_diff_eq(Vec3::new(1.0, 0.9, 1.0), 1e-5),
        "the point on the right edge straight across, got {snapped}"
    );
}

fn cube_object(scale: Scalar) -> SceneObject {
    let mesh = Arc::new(unit_cube());
    let summary = crate::scene::ImportSummary {
        vertices_merged: 0,
        faces_removed: 0,
        orientation: core_geometry::Orientation {
            flipped_faces: 0,
            inverted_shells: 0,
            orientable: true,
        },
        diagnostics: core_geometry::diagnose(&mesh),
    };
    let transform = Transform {
        scale: Vec3::splat(scale),
        ..Transform::default()
    };
    let mut scene = crate::scene::Scene::default();
    let id = scene.insert(crate::scene::Imported::new(
        "cube".to_owned(),
        mesh,
        transform,
        summary,
    ));
    scene.get(id).expect("just inserted").clone()
}

#[test]
fn opposite_sides_of_a_cube_are_parallel_its_side_apart() {
    let cube = cube_object(10.0);
    let adjacency = Adjacency::of(&cube.mesh);
    let (bottom, top) = (0, 2);
    let mut measure = Measure::default();
    measure.pick_plane(Plane::of(&cube, &adjacency, bottom).expect("a flat side"));
    measure.pick_plane(Plane::of(&cube, &adjacency, top).expect("a flat side"));

    let gap = measure.plane_gap().expect("two planes picked");
    assert!(
        gap.is_parallel(),
        "opposite sides, got {} deg",
        gap.angle_deg
    );
    assert!(
        (gap.gap_mm - 10.0).abs() < 1e-4,
        "a 10 mm cube is 10 mm through, got {}",
        gap.gap_mm
    );
}

#[test]
fn neighbouring_sides_of_a_cube_stand_square() {
    let cube = cube_object(1.0);
    let adjacency = Adjacency::of(&cube.mesh);
    let (bottom, front) = (0, 4);
    let mut measure = Measure::default();
    measure.pick_plane(Plane::of(&cube, &adjacency, bottom).expect("a flat side"));
    measure.pick_plane(Plane::of(&cube, &adjacency, front).expect("a flat side"));

    let gap = measure.plane_gap().expect("two planes picked");
    assert!(
        (gap.angle_deg - 90.0).abs() < 1e-3,
        "sides of a cube meet square, got {}",
        gap.angle_deg
    );
    assert!(!gap.is_parallel());
    assert_eq!(
        (gap.from, gap.to),
        (Vec3::new(0.5, 0.0, 0.5), Vec3::new(0.5, 0.5, 0.0)),
        "a tilted pair is drawn centre to centre"
    );
}

/// Snaps on the unit cube, placed by `transform`, within half a millimetre.
fn snap_on_cube(transform: Transform, point: Vec3, to: Snap) -> (Vec3, Snapped) {
    let mesh = Arc::new(unit_cube());
    let bvh = Bvh::build(&mesh);
    snap(
        &mesh,
        &bvh,
        &Adjacency::of(&mesh),
        transform,
        point,
        0.5,
        to,
    )
}

#[test]
fn a_click_near_a_corner_measures_the_corner() {
    let near_corner = Vec3::new(0.98, 0.99, 1.0);

    let (snapped, by) = snap_on_cube(Transform::default(), near_corner, Snap::Corner);
    assert_eq!(by, Snapped::Corner);
    assert!(
        snapped.abs_diff_eq(Vec3::new(1.0, 1.0, 1.0), 1e-5),
        "the corner of the unit cube, got {snapped}"
    );
}

#[test]
fn a_click_in_the_middle_of_a_face_stays_where_it_landed() {
    // A 10 mm cube, on the diagonal its top is split along, which is no edge of it.
    let middle = Vec3::new(5.0, 5.0, 10.0);
    let ten = Transform {
        scale: Vec3::splat(10.0),
        ..Transform::default()
    };

    let (snapped, by) = snap_on_cube(ten, middle, Snap::Edge);
    assert_eq!(by, Snapped::Free);
    assert!(
        snapped.abs_diff_eq(middle, 1e-6),
        "no edge is within reach, and the diagonal seam is none"
    );
}

#[test]
fn a_corner_is_measured_where_the_model_stands() {
    let placed = Transform {
        translation: Vec3::new(10.0, 0.0, 0.0),
        scale: Vec3::splat(2.0),
        ..Transform::default()
    };

    let (snapped, _) = snap_on_cube(placed, Vec3::new(11.9, 1.9, 2.0), Snap::Corner);
    assert!(
        snapped.abs_diff_eq(Vec3::new(12.0, 2.0, 2.0), 1e-5),
        "the corner in plate coordinates, got {snapped}"
    );
}

#[test]
fn a_flattened_model_cannot_be_snapped_to() {
    let flat = Transform {
        scale: Vec3::new(1.0, 1.0, 0.0),
        ..Transform::default()
    };

    let point = Vec3::new(0.99, 0.99, 0.0);
    assert_eq!(
        snap_on_cube(flat, point, Snap::Edge),
        (point, Snapped::Free)
    );
}

#[test]
fn an_edge_out_of_reach_is_still_found_further_out() {
    // 0.8 mm in from the front edge: past the 0.5 mm reach, inside four times it.
    let ten = Transform {
        scale: Vec3::splat(10.0),
        ..Transform::default()
    };
    let (snapped, by) = snap_on_cube(ten, Vec3::new(5.0, 0.8, 10.0), Snap::Edge);
    assert_eq!(by, Snapped::Edge);
    assert!(
        snapped.abs_diff_eq(Vec3::new(5.0, 0.0, 10.0), 1e-4),
        "got {snapped}"
    );
}

#[test]
fn a_click_near_an_edge_measures_the_edge() {
    // 0.1 mm in from the top front edge of the cube, half way along it.
    let (snapped, by) = snap_on_cube(Transform::default(), Vec3::new(0.5, 0.1, 1.0), Snap::Edge);
    assert_eq!(by, Snapped::Edge);
    assert!(
        snapped.abs_diff_eq(Vec3::new(0.5, 0.0, 1.0), 1e-5),
        "the point on the edge straight across, got {snapped}"
    );
}
