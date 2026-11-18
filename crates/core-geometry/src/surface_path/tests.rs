use std::ops::ControlFlow;

use super::*;
use crate::{Bvh, ball, cuboid};

/// The point of `mesh` nearest `point`, with the face it lies on, unplaced.
fn on(mesh: &Mesh, point: Vec3) -> SurfacePoint {
    let closest = Bvh::build(mesh).closest(point).expect("the mesh has faces");
    SurfacePoint {
        face: closest.face,
        point: closest.point,
    }
}

fn walk(mesh: &Mesh, from: Vec3, to: Vec3) -> SurfacePath {
    surface_path(
        mesh,
        &Adjacency::of(mesh),
        Mat4::IDENTITY,
        on(mesh, from),
        on(mesh, to),
        &mut |_| ControlFlow::Continue(()),
    )
    .expect("both points are on one closed body")
}

fn ten_mm_cube() -> Mesh {
    cuboid(Vec3::ZERO, Vec3::splat(10.0))
}

#[test]
fn a_walk_from_the_top_of_a_cube_to_a_side_runs_over_the_edge() {
    let path = walk(
        &ten_mm_cube(),
        Vec3::new(5.0, 5.0, 10.0),
        Vec3::new(5.0, 0.0, 5.0),
    );
    assert!(
        (path.length_mm() - 10.0).abs() < 1e-3,
        "5 mm to the edge and 5 mm down, 10 mm; got {}",
        path.length_mm()
    );
}

#[test]
fn a_slanting_walk_over_a_cube_edge_is_straight_once_unfolded() {
    // The side x = 10 hinged up into the top's plane puts (10, 8, 3) at (17, 8).
    let path = walk(
        &ten_mm_cube(),
        Vec3::new(2.0, 5.0, 10.0),
        Vec3::new(10.0, 8.0, 3.0),
    );
    let expected = 234.0f32.sqrt();
    assert!(
        (path.length_mm() - expected).abs() < 1e-3,
        "expected {expected}, got {}",
        path.length_mm()
    );
}

#[test]
fn a_walk_from_the_top_of_a_cube_to_the_bottom_goes_down_a_side() {
    let path = walk(
        &ten_mm_cube(),
        Vec3::new(5.0, 5.0, 10.0),
        Vec3::new(5.0, 5.0, 0.0),
    );
    assert!(
        (path.length_mm() - 20.0).abs() < 1e-3,
        "5 to the edge, 10 down, 5 in: 20 mm; got {}",
        path.length_mm()
    );
}

#[test]
fn a_walk_on_a_ball_from_pole_to_equator_is_a_quarter_meridian() {
    let radius = 20.0;
    let mesh = ball(Vec3::ZERO, radius, 32, 64);
    let path = walk(&mesh, Vec3::Z * radius, Vec3::X * radius);
    let expected = std::f32::consts::FRAC_PI_2 * radius;
    assert!(
        (path.length_mm() - expected).abs() < expected * 0.01,
        "a quarter meridian of radius {radius} is {expected}, got {}",
        path.length_mm()
    );
}

#[test]
fn every_point_of_a_walk_lies_on_the_surface() {
    let mesh = ball(Vec3::ZERO, 10.0, 16, 32);
    let path = walk(&mesh, Vec3::new(10.0, 0.0, 0.0), Vec3::new(-6.0, 6.0, 5.0));
    let bvh = Bvh::build(&mesh);
    for point in &path.points {
        let off_mm = bvh.closest(*point).expect("the mesh has faces").distance;
        assert!(off_mm < 1e-4, "{point} is {off_mm} mm off the surface");
    }
}

#[test]
fn two_points_on_one_face_are_joined_straight() {
    let mesh = ten_mm_cube();
    let (from, to) = (Vec3::new(1.0, 1.0, 10.0), Vec3::new(2.0, 1.5, 10.0));
    let path = walk(&mesh, from, to);
    assert_eq!(path.points.len(), 2, "no edge crossed: {:?}", path.points);
    assert!((path.length_mm() - from.distance(to)).abs() < 1e-4);
}

#[test]
fn a_placed_mesh_is_walked_where_it_stands() {
    let mesh = ten_mm_cube();
    let placement = Mat4::from_scale_rotation_translation(
        Vec3::splat(2.0),
        crate::Quat::IDENTITY,
        Vec3::new(50.0, 0.0, 0.0),
    );
    let lift = |at: SurfacePoint| SurfacePoint {
        point: placement.transform_point3(at.point),
        ..at
    };
    let from = lift(on(&mesh, Vec3::new(5.0, 5.0, 10.0)));
    let to = lift(on(&mesh, Vec3::new(5.0, 5.0, 0.0)));
    let path = surface_path(
        &mesh,
        &Adjacency::of(&mesh),
        placement,
        from,
        to,
        &mut |_| ControlFlow::Continue(()),
    )
    .expect("both points are on the cube");
    assert!(
        (path.length_mm() - 40.0).abs() < 1e-3,
        "twice the 20 mm of the unplaced cube; got {}",
        path.length_mm()
    );
}

#[test]
fn a_face_past_the_mesh_is_out_of_range() {
    let mesh = ten_mm_cube();
    let inside = on(&mesh, Vec3::new(5.0, 5.0, 10.0));
    let past = SurfacePoint { face: 12, ..inside };
    let found = surface_path(
        &mesh,
        &Adjacency::of(&mesh),
        Mat4::IDENTITY,
        inside,
        past,
        &mut |_| ControlFlow::Continue(()),
    );
    assert!(matches!(
        found,
        Err(SurfacePathError::FaceOutOfRange { face: 12, .. })
    ));
}

#[test]
fn two_bodies_apart_have_no_walk_between_them() {
    let mut mesh = ten_mm_cube();
    mesh.append(&cuboid(Vec3::splat(20.0), Vec3::splat(30.0)));
    let from = on(&mesh, Vec3::new(5.0, 5.0, 10.0));
    let to = on(&mesh, Vec3::new(25.0, 25.0, 30.0));
    let found = surface_path(
        &mesh,
        &Adjacency::of(&mesh),
        Mat4::IDENTITY,
        from,
        to,
        &mut |_| ControlFlow::Continue(()),
    );
    assert!(matches!(found, Err(SurfacePathError::Unreachable { .. })));
}

#[test]
fn halfway_down_a_cube_from_top_to_bottom_is_halfway_down_a_side() {
    let path = walk(
        &ten_mm_cube(),
        Vec3::new(5.0, 5.0, 10.0),
        Vec3::new(5.0, 5.0, 0.0),
    );
    let halfway = path.halfway().expect("the walk has points");
    assert!(
        (halfway.z - 5.0).abs() < 1e-3,
        "10 of the 20 mm is 5 mm down a side; got {halfway}"
    );
}

#[test]
fn a_walk_given_up_on_stops() {
    let mesh = ten_mm_cube();
    let found = surface_path(
        &mesh,
        &Adjacency::of(&mesh),
        Mat4::IDENTITY,
        on(&mesh, Vec3::new(5.0, 5.0, 10.0)),
        on(&mesh, Vec3::new(5.0, 5.0, 0.0)),
        &mut |_| ControlFlow::Break(()),
    );
    assert!(matches!(found, Err(SurfacePathError::Stopped)));
}

#[test]
fn a_walk_tells_how_far_it_has_got_from_none_of_the_way() {
    let mesh = ball(Vec3::ZERO, 20.0, 64, 128);
    let mut told = Vec::new();
    surface_path(
        &mesh,
        &Adjacency::of(&mesh),
        Mat4::IDENTITY,
        on(&mesh, Vec3::X * 20.0),
        on(&mesh, Vec3::new(-20.0, 0.0, 1.0)),
        &mut |share| {
            told.push(share);
            ControlFlow::Continue(())
        },
    )
    .expect("a ball is one closed body");
    assert_eq!(
        told.first(),
        Some(&0.0),
        "nothing found before the first step"
    );
    assert!(
        told.windows(2).all(|pair| pair[0] <= pair[1]),
        "the share never goes back: {told:?}"
    );
}
