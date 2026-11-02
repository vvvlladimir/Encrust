use super::*;
use core_geometry::cuboid;

/// A 20 mm cube with a 10 mm cube of air sealed in its middle, the cavity wound inward as a
/// hollowed model carries it: a 5 mm wall all round.
fn sealed_cube() -> Mesh {
    let mut mesh = cuboid(Vec3::ZERO, Vec3::splat(20.0));
    let cavity = cuboid(Vec3::splat(5.0), Vec3::splat(15.0));
    let inward = Mesh::new(
        cavity.vertices,
        cavity.faces.iter().map(|[a, b, c]| [*a, *c, *b]).collect(),
    );
    mesh.append(&inward);
    mesh
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
    let holes = holes_out(&mesh, &bvh, FLOOR, Toward::Down, three_mm(), None);
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
    let holes = holes_out(&mesh, &bvh, FLOOR, Toward::Down, three_mm(), Some(0.0));
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
    let holes = holes_out(&mesh, &bvh, FLOOR, Toward::Up, three_mm(), Some(0.0));
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
    let holes = holes_out(&mesh, &bvh, FLOOR, Toward::Down, wide, None);
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
    assert!(holes_out(&mesh, &bvh, above, Toward::Down, three_mm(), None).is_empty());
}
