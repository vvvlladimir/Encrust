use super::*;
use core_geometry::{Vec3, cuboid};

/// Cubes from `low` to `high`, each with the owner and kind given.
fn bodies_of(cubes: &[(usize, BodyKind, Vec3, Vec3)]) -> Vec<Mesh> {
    cubes
        .iter()
        .map(|&(_, _, low, high)| cuboid(low, high))
        .collect()
}

fn met(cubes: &[(usize, BodyKind, Vec3, Vec3)]) -> Vec<[usize; 2]> {
    let meshes = bodies_of(cubes);
    let bodies: Vec<Body<'_>> = cubes
        .iter()
        .zip(&meshes)
        .map(|(&(owner, kind, _, _), mesh)| Body {
            owner,
            kind,
            mesh,
            bvh: None,
            transform: Transform::default(),
        })
        .collect();
    crossings(&bodies)
}

use BodyKind::{Model, Supports};

#[test]
fn two_cubes_overlapping_meet_and_two_apart_do_not() {
    let met = met(&[
        (0, Model, Vec3::ZERO, Vec3::splat(10.0)),
        (1, Model, Vec3::splat(5.0), Vec3::splat(15.0)),
        (
            2,
            Model,
            Vec3::new(40.0, 0.0, 0.0),
            Vec3::new(50.0, 10.0, 10.0),
        ),
    ]);
    assert_eq!(met, vec![[0, 1]], "only the first two share x = 5..10 mm");
}

#[test]
fn a_cube_wholly_inside_another_meets_it() {
    let met = met(&[
        (0, Model, Vec3::ZERO, Vec3::splat(20.0)),
        (1, Model, Vec3::splat(8.0), Vec3::splat(12.0)),
    ]);
    assert_eq!(
        met,
        vec![[0, 1]],
        "no face crosses, but the small one is inside"
    );
}

#[test]
fn a_cube_in_the_corner_of_anothers_box_but_clear_of_it_does_not_meet() {
    let met = met(&[
        (0, Model, Vec3::ZERO, Vec3::splat(10.0)),
        (
            1,
            Model,
            Vec3::new(10.5, 0.0, 0.0),
            Vec3::new(20.0, 10.0, 10.0),
        ),
    ]);
    assert!(met.is_empty(), "half a millimetre apart, got {met:?}");
}

#[test]
fn supports_reaching_into_another_model_meet_it() {
    let met = met(&[
        (
            0,
            Model,
            Vec3::new(0.0, 0.0, 5.0),
            Vec3::new(10.0, 10.0, 15.0),
        ),
        (0, Supports, Vec3::ZERO, Vec3::new(14.0, 10.0, 5.0)),
        (
            1,
            Model,
            Vec3::new(12.0, 0.0, 0.0),
            Vec3::new(22.0, 10.0, 10.0),
        ),
    ]);
    assert_eq!(
        met,
        vec![[0, 1]],
        "the raft runs to x = 14 mm, into x = 12..22 mm"
    );
}

#[test]
fn supports_meeting_supports_and_a_models_own_supports_are_not_a_meeting() {
    let met = met(&[
        (
            0,
            Model,
            Vec3::new(0.0, 0.0, 5.0),
            Vec3::new(10.0, 10.0, 15.0),
        ),
        (
            0,
            Supports,
            Vec3::new(-2.0, -2.0, 0.0),
            Vec3::new(12.0, 12.0, 6.0),
        ),
        (
            1,
            Model,
            Vec3::new(14.0, 0.0, 5.0),
            Vec3::new(24.0, 10.0, 15.0),
        ),
        (
            1,
            Supports,
            Vec3::new(11.0, -2.0, 0.0),
            Vec3::new(26.0, 12.0, 5.0),
        ),
    ]);
    assert!(
        met.is_empty(),
        "the rafts overlap at x = 11..12 mm only, got {met:?}"
    );
}
