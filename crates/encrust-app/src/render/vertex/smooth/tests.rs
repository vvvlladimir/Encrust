use super::*;
use core_geometry::{ball, cuboid, cylinder};

#[test]
fn a_box_keeps_its_edges_with_a_corner_laid_once_for_each_face_meeting_there() {
    let cube = cuboid(Vec3::ZERO, Vec3::ONE);
    let (vertices, indices) = smooth_vertices(&cube, 0..cube.faces.len());

    assert_eq!(
        vertices.len(),
        24,
        "eight corners, each on three faces at right angles"
    );
    for vertex in &vertices {
        let normal = Vec3::from_array(vertex.normal);
        let axis = normal.abs().max_element();
        assert!(
            (axis - 1.0).abs() < 1e-5,
            "a face of a box is shaded flat, along an axis, got {normal}"
        );
    }
    assert_eq!(indices.len(), cube.faces.len() * 3);
}

#[test]
fn a_ball_is_shaded_by_normals_pointing_out_of_its_centre() {
    let centre = Vec3::new(1.0, 2.0, 3.0);
    let sphere = ball(centre, 2.0, 16, 24);
    let (vertices, _) = smooth_vertices(&sphere, 0..sphere.faces.len());

    for vertex in &vertices {
        let out = (Vec3::from_array(vertex.position) - centre).normalize();
        let normal = Vec3::from_array(vertex.normal);
        assert!(
            normal.dot(out) > 0.99,
            "a corner of a ball is shaded along its radius {out}, got {normal}"
        );
    }
}

#[test]
fn the_side_of_a_tube_blends_and_its_flat_ends_stay_flat() {
    let tube = cylinder(Vec3::ZERO, Vec3::new(0.0, 0.0, 5.0), 1.0, 12);
    let (vertices, _) = smooth_vertices(&tube, 0..tube.faces.len());

    let radial = vertices.iter().filter(|vertex| {
        let normal = Vec3::from_array(vertex.normal);
        let position = Vec3::from_array(vertex.position);
        normal.z.abs() < 1e-5 && normal.dot(position.truncate().extend(0.0).normalize()) > 0.999
    });
    let capped = vertices
        .iter()
        .filter(|vertex| (vertex.normal[2].abs() - 1.0).abs() < 1e-5);
    assert!(
        radial.count() >= 24,
        "every rim corner is shaded along its radius on the side"
    );
    assert!(
        capped.count() >= 24,
        "and straight up or down on the end it closes"
    );
}

#[test]
fn a_face_with_a_corner_twice_still_points_every_index_at_a_vertex() {
    let mut pinched = cuboid(Vec3::ZERO, Vec3::ONE);
    pinched.faces.push([0, 0, 1]);
    let (vertices, indices) = smooth_vertices(&pinched, 0..pinched.faces.len());

    assert!(
        indices
            .iter()
            .all(|index| (*index as usize) < vertices.len()),
        "no index runs past the corners laid out"
    );
}

#[test]
fn a_face_of_no_area_neither_lays_a_corner_nor_bends_one() {
    let cube = cuboid(Vec3::ZERO, Vec3::ONE);
    let mut pinched = cube.clone();
    pinched.faces.push([0, 0, 1]);
    let (plain, _) = smooth_vertices(&cube, 0..cube.faces.len());
    let (vertices, _) = smooth_vertices(&pinched, 0..pinched.faces.len());

    assert_eq!(
        vertices.len(),
        plain.len(),
        "no corner laid for a face of no area"
    );
    assert!(
        vertices
            .iter()
            .zip(&plain)
            .all(|(one, other)| one.normal == other.normal),
        "and none of the cube's corners leans"
    );
}
