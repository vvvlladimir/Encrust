use super::*;
use crate::fixtures::sealed;

/// A 20 mm cube with a 10 mm cube of air sealed in its middle: a 5 mm wall all round.
fn sealed_cube() -> Mesh {
    sealed(
        (Vec3::ZERO, Vec3::splat(20.0)),
        &[(Vec3::splat(5.0), Vec3::splat(15.0))],
    )
}

/// A 24 mm plate 4 mm thick round a cavity 1 mm high: a 1.5 mm wall under and over it.
fn thin_plate() -> Mesh {
    sealed(
        (Vec3::ZERO, Vec3::new(24.0, 24.0, 4.0)),
        &[(Vec3::new(1.5, 1.5, 1.5), Vec3::new(22.5, 22.5, 2.5))],
    )
}

fn three_mm() -> HoleSize {
    HoleSize::round(3.0, 1.0, 1.0)
}

/// The cavity's floor, where the drainage scan puts a pocket's point.
const FLOOR: Vec3 = Vec3::new(10.0, 10.0, 5.25);

#[test]
fn a_lifted_cube_drains_straight_down_through_its_floor() {
    let mesh = sealed_cube();
    let bvh = Bvh::build(&mesh);
    let holes = holes_out(&mesh, &bvh, FLOOR, Toward::Down, three_mm(), None, None);
    let best = holes.first().expect("a cube's floor is a face to drill");
    assert!(
        best.at.distance(Vec3::new(10.0, 10.0, 0.0)) < 1e-3,
        "the lowest way out is the middle of the bottom face, (10, 10, 0), got {}",
        best.at
    );
    assert!(
        best.axis.normalize().dot(Vec3::Z) > 0.999,
        "drilled straight up into the floor, got {}",
        best.axis
    );
    assert!(
        best.depth_mm >= 5.0 + 3.0,
        "a diameter past the 5 mm wall, so at least 8 mm, got {}",
        best.depth_mm
    );
}

#[test]
fn a_cube_on_the_plate_drains_through_a_side_level_with_its_floor() {
    let mesh = sealed_cube();
    let bvh = Bvh::build(&mesh);
    let holes = holes_out(
        &mesh,
        &bvh,
        FLOOR,
        Toward::Down,
        three_mm(),
        Some(0.0),
        None,
    );
    assert_eq!(
        holes.len(),
        4,
        "the four sides, square to the floor's middle; the plate takes the rest, got {holes:?}"
    );
    for hole in &holes {
        assert!(
            (hole.at.z - 6.75).abs() < 1e-3,
            "a radius over the floor at 5.25, so 6.75 mm up, got {}",
            hole.at
        );
        assert!(hole.axis.z.abs() < 1e-3, "drilled level, got {}", hole.axis);
    }
}

#[test]
fn a_vent_opens_in_the_top_face() {
    let mesh = sealed_cube();
    let bvh = Bvh::build(&mesh);
    let holes = holes_out(&mesh, &bvh, FLOOR, Toward::Up, three_mm(), Some(0.0), None);
    let best = holes.first().expect("a cube's top is a face to drill");
    assert!(
        best.at.distance(Vec3::new(10.0, 10.0, 20.0)) < 1e-3,
        "the highest way out is the middle of the top, (10, 10, 20), got {}",
        best.at
    );
}

#[test]
fn a_hole_wider_than_every_face_has_no_way_out() {
    let mesh = sealed_cube();
    let bvh = Bvh::build(&mesh);
    let wide = HoleSize::round(24.0, 1.0, 1.0);
    let holes = holes_out(&mesh, &bvh, FLOOR, Toward::Down, wide, None, None);
    assert!(
        holes.is_empty(),
        "a 24 mm hole is wider than a 20 mm face, got {holes:?}"
    );
}

#[test]
fn a_point_over_nothing_is_in_no_cavity() {
    let mesh = sealed_cube();
    let bvh = Bvh::build(&mesh);
    let above = Vec3::new(10.0, 10.0, 30.0);
    assert!(holes_out(&mesh, &bvh, above, Toward::Down, three_mm(), None, None).is_empty());
}

/// The thin plate's cavity floor, where the drainage scan puts its point.
const THIN_FLOOR: Vec3 = Vec3::new(12.0, 12.0, 1.75);

#[test]
fn a_hole_into_a_thin_cavity_stops_short_of_its_far_side() {
    let mesh = thin_plate();
    let bvh = Bvh::build(&mesh);
    let wall = Some((1.5, 0.1));
    let holes = holes_out(
        &mesh,
        &bvh,
        THIN_FLOOR,
        Toward::Down,
        three_mm(),
        None,
        wall,
    );
    let best = holes
        .first()
        .expect("the plate's underside is a face to drill");
    assert!(
        best.at.distance(Vec3::new(12.0, 12.0, 0.0)) < 1e-3 && best.axis.z > 0.999,
        "drilled straight up through the middle of the underside, got {best:?}"
    );
    // The ceiling is 2.5 mm up: the hole stops the 0.5 mm clearance under it.
    assert!(
        (best.depth_mm - 2.0).abs() < 1e-3,
        "expected 2.0 mm deep, past the 1.5 mm floor and short of the 2.5 mm ceiling, got {}",
        best.depth_mm
    );
    for hole in &holes {
        let tip = hole.at + hole.axis.normalize() * hole.depth_mm;
        assert!(
            hole.axis.z < 0.5 || tip.z <= 2.0 + 1e-3,
            "no hole up through the floor reaches within 0.5 mm of the ceiling at 2.5, got {hole:?}"
        );
    }
}

#[test]
fn a_cavity_too_thin_to_stop_in_is_not_drilled_across() {
    let mesh = thin_plate();
    let bvh = Bvh::build(&mesh);
    // Carried 1.5 mm and two 0.3 mm cells deep, 2.1 mm, a hole would pass the 2.0 mm it may.
    let wall = Some((1.5, 0.3));
    let holes = holes_out(
        &mesh,
        &bvh,
        THIN_FLOOR,
        Toward::Down,
        three_mm(),
        None,
        wall,
    );
    assert!(
        holes.iter().all(|hole| hole.axis.z.abs() < 1e-3),
        "only level holes, across the 21 mm of room, got {holes:?}"
    );
}
