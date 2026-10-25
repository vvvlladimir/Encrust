use super::*;
use crate::fixtures::box_mesh;
use core_geometry::{Bvh, Mesh, Transform};

/// A wall filling 0..1 on x, standing beside the origin.
fn wall() -> Mesh {
    box_mesh(Vec3::new(0.0, -10.0, 0.0), Vec3::new(1.0, 10.0, 20.0))
}

/// A beam of one radius, run against the wall.
fn clears(from: Vec3, to: Vec3, radius_mm: Scalar) -> bool {
    let model = wall();
    clear(
        &Placed::new(&model, &Bvh::build(&model), Transform::default()),
        &Beam {
            from,
            to,
            from_radius_mm: radius_mm,
            to_radius_mm: radius_mm,
        },
    )
}

#[test]
fn a_body_well_clear_of_the_model_passes() {
    assert!(clears(
        Vec3::new(5.0, 0.0, 15.0),
        Vec3::new(5.0, 0.0, 1.0),
        0.4
    ));
}

#[test]
fn a_body_running_alongside_a_wall_within_its_own_radius_is_blocked() {
    // Axis 0.3 mm clear of the wall's face at x = 1, body 0.4 mm across: the axis
    // ray never meets the wall and the body is inside it.
    assert!(!clears(
        Vec3::new(1.3, 0.0, 15.0),
        Vec3::new(1.3, 0.0, 1.0),
        0.4
    ));
}

#[test]
fn a_body_crossing_the_model_is_blocked() {
    assert!(!clears(
        Vec3::new(-2.0, 0.0, 10.0),
        Vec3::new(3.0, 0.0, 10.0),
        0.1
    ));
}

#[test]
fn a_body_starting_inside_the_model_is_blocked() {
    assert!(!clears(
        Vec3::new(0.5, 0.0, 10.0),
        Vec3::new(0.5, 0.0, 1.0),
        0.1
    ));
}

#[test]
fn a_body_of_no_length_meets_nothing() {
    let at = Vec3::new(0.5, 0.0, 10.0);
    assert!(clears(at, at, 0.4), "there is no body to sweep");
}

#[test]
fn clearance_pushes_a_body_away_from_a_wall_it_was_passing() {
    let model = wall();
    let bvh = Bvh::build(&model);
    let beam = Beam {
        from: Vec3::new(1.5, 0.0, 15.0),
        to: Vec3::new(1.5, 0.0, 1.0),
        from_radius_mm: 0.2,
        to_radius_mm: 0.2,
    };

    assert!(clear(
        &Placed::new(&model, &bvh, Transform::default()),
        &beam
    ));
    assert!(
        !clear(
            &Placed::new(&model, &bvh, Transform::default()),
            &beam.with_clearance(0.5)
        ),
        "half a millimetre of air does not fit in the 0.3 mm left beside the wall"
    );
}

#[test]
fn a_trimmed_beam_starts_past_what_was_taken_off_it() {
    let beam = Beam {
        from: Vec3::new(0.0, 0.0, 10.0),
        to: Vec3::new(0.0, 0.0, 0.0),
        from_radius_mm: 0.2,
        to_radius_mm: 0.4,
    };
    let trimmed = beam.trimmed(5.0);

    assert!((trimmed.from.z - 5.0).abs() < 1e-5);
    assert!(
        (trimmed.from_radius_mm - 0.3).abs() < 1e-5,
        "the radius is read off the taper where the trim ends"
    );
    assert!((trimmed.to_radius_mm - 0.4).abs() < 1e-5);
}

#[test]
fn trimming_past_the_far_end_leaves_no_body() {
    let beam = Beam {
        from: Vec3::new(1.3, 0.0, 15.0),
        to: Vec3::new(1.3, 0.0, 1.0),
        from_radius_mm: 0.4,
        to_radius_mm: 0.4,
    };
    let model = wall();
    assert!(clear(
        &Placed::new(&model, &Bvh::build(&model), Transform::default()),
        &beam.trimmed(100.0)
    ));
}

#[test]
fn a_rod_standing_inside_a_body_between_its_rays_is_blocked() {
    // A rod 0.1 mm square, 0.5 mm off the axis of a body 1 mm in radius: parallel to
    // every ray, it lies between the axis and the ring and meets none of them.
    let model = box_mesh(Vec3::new(0.45, -0.05, 5.0), Vec3::new(0.55, 0.05, 10.0));
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let beam = Beam {
        from: Vec3::new(0.0, 0.0, 15.0),
        to: Vec3::new(0.0, 0.0, 1.0),
        from_radius_mm: 1.0,
        to_radius_mm: 1.0,
    };
    assert!(rays_clear(&placed, &beam), "the rays alone pass it by");
    assert!(!clear(&placed, &beam), "the balls down the axis find it");
}

#[test]
fn a_body_ending_on_the_face_it_lands_on_is_clear() {
    // Standing straight on the wall's top at z = 20, as a landing on the model does.
    let model = wall();
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let beam = Beam {
        from: Vec3::new(0.5, 0.0, 30.0),
        to: Vec3::new(0.5, 0.0, 20.0 + 1e-3),
        from_radius_mm: 0.4,
        to_radius_mm: 0.4,
    };
    assert!(clear(&placed, &beam));
}

#[test]
fn a_ball_is_measured_on_the_plate_through_a_stretched_placement() {
    // Doubled along x, the wall's face stands at x = 2 on the plate.
    let model = wall();
    let bvh = Bvh::build(&model);
    let transform = Transform {
        scale: Vec3::new(2.0, 1.0, 1.0),
        ..Transform::default()
    };
    let placed = Placed::new(&model, &bvh, transform);
    let mut found = Vec::new();
    assert!(placed.reaches_ball(Vec3::new(2.4, 0.0, 10.0), 0.5, &mut found));
    assert!(!placed.reaches_ball(Vec3::new(2.6, 0.0, 10.0), 0.5, &mut found));
}
