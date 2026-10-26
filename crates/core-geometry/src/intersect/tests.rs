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
