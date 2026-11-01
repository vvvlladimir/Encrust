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
    meetings(cubes)
        .iter()
        .map(|meeting| meeting.owners)
        .collect()
}

fn meetings(cubes: &[(usize, BodyKind, Vec3, Vec3)]) -> Vec<Meeting> {
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
fn two_cubes_meet_in_one_place_within_both() {
    let met = meetings(&[
        (0, Model, Vec3::ZERO, Vec3::splat(10.0)),
        (
            1,
            Model,
            Vec3::new(40.0, 0.0, 0.0),
            Vec3::new(50.0, 10.0, 10.0),
        ),
        (
            2,
            Model,
            Vec3::new(5.0, 2.0, 1.0),
            Vec3::new(15.0, 8.0, 9.0),
        ),
    ]);
    assert_eq!(met.len(), 1);
    assert_eq!(met[0].owners, [0, 2]);
    assert_eq!(met[0].places_mm.len(), 1, "one seam round the overlap");
    assert!(
        !met[0].seam_mm.is_empty(),
        "walls crossing at an angle cut a seam"
    );
    let place = met[0].places_mm[0];
    // The second's wall at x = 5 stands wholly inside the first, so the seam is where the
    // first's wall at x = 10 cuts through the second: a frame over y = 2..8, z = 1..9.
    let (low, high) = (Vec3::new(10.0, 2.0, 1.0), Vec3::new(10.0, 8.0, 9.0));
    assert!(
        (place.mins - low).abs().max_element() < 1e-4
            && (place.maxs - high).abs().max_element() < 1e-4,
        "expected {low}..{high}, got {}..{}",
        place.mins,
        place.maxs
    );
}

#[test]
fn a_raft_reaching_into_a_model_at_two_ends_meets_it_in_two_places() {
    let met = meetings(&[
        (
            0,
            Model,
            Vec3::new(10.0, 0.0, 5.0),
            Vec3::new(20.0, 10.0, 15.0),
        ),
        (
            1,
            Model,
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::new(4.0, 8.0, 10.0),
        ),
        (
            1,
            Model,
            Vec3::new(26.0, 2.0, 0.0),
            Vec3::new(30.0, 8.0, 10.0),
        ),
        (
            0,
            Supports,
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(28.0, 10.0, 3.0),
        ),
    ]);
    assert_eq!(met.len(), 1);
    assert_eq!(
        met[0].places_mm.len(),
        2,
        "the raft cuts each block, 22 mm apart"
    );
}

#[test]
fn a_cube_wholly_inside_another_meets_it_where_it_stands() {
    let met = meetings(&[
        (0, Model, Vec3::ZERO, Vec3::splat(20.0)),
        (1, Model, Vec3::splat(8.0), Vec3::splat(12.0)),
    ]);
    assert_eq!(met.len(), 1);
    let place = met[0].places_mm[0];
    assert_eq!(
        (place.mins, place.maxs),
        (Vec3::splat(8.0), Vec3::splat(12.0)),
        "no face crosses, so it meets where the small cube stands"
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
