use super::*;
use crate::{Transform, Vec3, ball, cuboid, transform_mesh};

/// Two meshes as one.
fn joined(one: Mesh, other: &Mesh) -> Mesh {
    let mut mesh = one;
    mesh.append(other);
    mesh
}

fn crossings(mesh: &Mesh) -> Vec<[u32; 2]> {
    self_intersections(mesh, &Bvh::build(mesh))
}

#[test]
fn a_closed_body_crosses_nothing() {
    assert!(crossings(&cuboid(Vec3::ZERO, Vec3::ONE)).is_empty());
    assert!(crossings(&ball(Vec3::ZERO, 10.0, 24, 48)).is_empty());
}

#[test]
fn two_bodies_apart_cross_nothing() {
    let mesh = joined(
        cuboid(Vec3::ZERO, Vec3::ONE),
        &cuboid(Vec3::splat(2.0), Vec3::splat(3.0)),
    );
    assert!(crossings(&mesh).is_empty());
}

#[test]
fn two_overlapping_cubes_cross_only_each_other() {
    let one = cuboid(Vec3::ZERO, Vec3::ONE);
    let shifted = Vec3::new(0.5, 0.3, 0.4);
    let mesh = joined(one.clone(), &cuboid(shifted, shifted + Vec3::ONE));
    let first_of_other = one.faces.len() as u32;

    let pairs = crossings(&mesh);

    assert!(
        !pairs.is_empty(),
        "their surfaces meet along a closed curve"
    );
    for [face, other] in &pairs {
        assert!(
            *face < first_of_other && *other >= first_of_other,
            "{face} and {other}"
        );
    }
}

#[test]
fn faces_lying_over_each_other_in_one_plane_cross() {
    // The second cube is the first slid half along x: its top and bottom overlap the first's.
    let one = cuboid(Vec3::ZERO, Vec3::ONE);
    let other = transform_mesh(&one, Transform::from_translation(Vec3::new(0.5, 0.0, 0.0)));
    let pairs = crossings(&joined(one, &other));

    assert!(!pairs.is_empty());
}

#[test]
fn faces_only_touching_along_an_edge_are_no_crossing() {
    let mesh = joined(
        cuboid(Vec3::ZERO, Vec3::ONE),
        &cuboid(Vec3::new(1.0, 0.0, 0.0), Vec3::new(2.0, 1.0, 1.0)),
    );
    let touching = crossings(&mesh)
        .iter()
        .filter(|[face, other]| {
            let normal = |face: u32| {
                mesh.triangle(face as usize)
                    .map(|t| t.normal_unnormalized().normalize())
            };
            normal(*face)
                .zip(normal(*other))
                .is_some_and(|(a, b)| a.dot(b).abs() < 0.5)
        })
        .count();
    assert_eq!(
        touching, 0,
        "a wall meeting a wall at its edge only touches it"
    );
}

/// `mesh` where `transform` stands it, its box read off its vertices.
fn placed<'a>(mesh: &'a Mesh, bvh: &'a Bvh, transform: Transform) -> PlacedMesh<'a> {
    let bounds_mm = crate::placed_aabb(mesh, transform).expect("a cube has vertices");
    PlacedMesh {
        mesh,
        bvh,
        transform,
        bounds_mm,
    }
}

fn cubes_cross(at: Transform) -> bool {
    let cube = cuboid(Vec3::ZERO, Vec3::ONE);
    let bvh = Bvh::build(&cube);
    let still = placed(&cube, &bvh, Transform::default());
    !placed_crossings(&still, &placed(&cube, &bvh, at)).is_empty()
}

#[test]
fn a_cube_moved_half_its_width_into_another_crosses_it() {
    assert!(cubes_cross(Transform::from_translation(Vec3::new(
        0.5, 0.25, 0.0
    ))));
}

#[test]
fn each_crossing_names_a_face_of_the_first_and_one_of_the_second_that_cross() {
    let cube = cuboid(Vec3::ZERO, Vec3::ONE);
    let bvh = Bvh::build(&cube);
    let still = placed(&cube, &bvh, Transform::default());
    let moved = Transform::from_translation(Vec3::new(0.5, 0.25, 0.1));
    let crossed = placed_crossings(&still, &placed(&cube, &bvh, moved));
    assert!(!crossed.is_empty());
    let placed_corners = |face: u32, at: Transform| {
        let matrix = at.to_matrix();
        cube.faces[face as usize].map(|corner| {
            matrix
                .transform_point3(cube.vertices[corner as usize])
                .as_dvec3()
        })
    };
    for Crossing {
        faces: [one, other],
        ..
    } in crossed
    {
        let (a, b) = (
            placed_corners(one, Transform::default()),
            placed_corners(other, moved),
        );
        assert!(
            cross(&a, &b, Rim::Closed),
            "faces {one} and {other} were named but do not cross"
        );
    }
}

#[test]
fn the_seam_of_two_cubes_lies_on_both_their_surfaces() {
    // Moved (0.5, 0.25, 0.1), the second cube's walls cut the first's along x = 1, y = 1
    // and z = 1, and the first's cut the second's along x = 0.5, y = 0.25 and z = 0.1.
    let cube = cuboid(Vec3::ZERO, Vec3::ONE);
    let bvh = Bvh::build(&cube);
    let moved = Transform::from_translation(Vec3::new(0.5, 0.25, 0.1));
    let still = placed(&cube, &bvh, Transform::default());
    let crossed = placed_crossings(&still, &placed(&cube, &bvh, moved));
    let on_a_wall = |point: Vec3, low: Vec3, high: Vec3| {
        let near = |a: f32, b: f32| (a - b).abs() < 1e-5;
        (0..3).any(|axis| near(point[axis], low[axis]) || near(point[axis], high[axis]))
    };
    let seams: Vec<[Vec3; 2]> = crossed
        .iter()
        .filter_map(|crossing| crossing.seam_mm)
        .collect();
    assert!(!seams.is_empty(), "walls crossing at an angle cut a seam");
    for end in seams.iter().flatten() {
        assert!(
            on_a_wall(*end, Vec3::ZERO, Vec3::ONE)
                && on_a_wall(*end, Vec3::new(0.5, 0.25, 0.1), Vec3::new(1.5, 1.25, 1.1)),
            "{end} lies on a wall of each cube"
        );
    }
}

#[test]
fn a_cube_moved_by_a_round_half_crosses_though_every_crossing_lands_on_a_rim() {
    // Half a width on every axis puts each crossing point on a face's diagonal.
    assert!(cubes_cross(Transform::from_translation(Vec3::splat(0.5))));
}

#[test]
fn a_cube_moved_clear_of_another_crosses_nothing() {
    assert!(!cubes_cross(Transform::from_translation(Vec3::new(
        1.5, 0.0, 0.0
    ))));
}

#[test]
fn a_turned_cube_whose_box_overlaps_but_whose_faces_stop_short_crosses_nothing() {
    // Turned 45° about Z and centred at (1.6, 1.6), its box reaches x = 1.6 - √2/2 = 0.89 mm,
    // inside the other's, but its face towards the corner (1, 1) stands 0.5 mm from its
    // centre and the corner 0.6·√2 = 0.85 mm.
    let turn = glam::Quat::from_rotation_z(std::f32::consts::FRAC_PI_4);
    let centre = Vec3::new(1.6, 1.6, 0.5);
    let at = Transform::from_scale_rotation_translation(
        Vec3::ONE,
        turn,
        centre - turn * Vec3::splat(0.5),
    );
    assert!(!cubes_cross(at));
}

#[test]
fn a_cube_wholly_inside_another_crosses_none_of_its_faces() {
    let big = cuboid(Vec3::ZERO, Vec3::splat(10.0));
    let small = cuboid(Vec3::splat(4.0), Vec3::splat(6.0));
    let (big_bvh, small_bvh) = (Bvh::build(&big), Bvh::build(&small));
    let outer = placed(&big, &big_bvh, Transform::default());
    let inner = placed(&small, &small_bvh, Transform::default());
    assert!(
        placed_crossings(&outer, &inner).is_empty(),
        "inside is not crossing"
    );
}
