use super::*;
use crate::{ball, cuboid, cylinder};

const CREASE_DEG: Scalar = 50.0;

fn normals_at(smooth: &SmoothNormals, vertex: u32) -> Vec<Vec3> {
    smooth
        .vertices
        .iter()
        .zip(&smooth.normals)
        .filter(|(at, _)| **at == vertex)
        .map(|(_, normal)| *normal)
        .collect()
}

#[test]
fn a_box_keeps_its_edges_with_a_normal_for_each_face_meeting_at_a_corner() {
    let cube = cuboid(Vec3::ZERO, Vec3::ONE);
    let smooth = smooth_normals(&cube, CREASE_DEG);

    assert_eq!(
        smooth.normals.len(),
        24,
        "eight corners, each on three faces at right angles"
    );
    for normal in &smooth.normals {
        assert!(
            (normal.abs().max_element() - 1.0).abs() < 1e-5,
            "a face of a box is shaded flat, along an axis, got {normal}"
        );
    }
}

#[test]
fn a_ball_is_shaded_by_normals_pointing_out_of_its_centre() {
    let centre = Vec3::new(1.0, 2.0, 3.0);
    let sphere = ball(centre, 2.0, 16, 24);
    let smooth = smooth_normals(&sphere, CREASE_DEG);

    assert_eq!(
        smooth.normals.len(),
        sphere.vertices.len(),
        "no edge of a fine ball is a crease, so each vertex has one normal"
    );
    for (vertex, normal) in smooth.vertices.iter().zip(&smooth.normals) {
        let out = (sphere.vertices[*vertex as usize] - centre).normalize();
        assert!(
            normal.dot(out) > 0.99,
            "a corner of a ball is shaded along its radius {out}, got {normal}"
        );
    }
}

#[test]
fn the_side_of_a_tube_blends_and_its_flat_ends_stay_flat() {
    let tube = cylinder(Vec3::ZERO, Vec3::new(0.0, 0.0, 5.0), 1.0, 12);
    let smooth = smooth_normals(&tube, CREASE_DEG);

    for (vertex, normal) in smooth.vertices.iter().zip(&smooth.normals) {
        let position = tube.vertices[*vertex as usize];
        let radial = position.truncate().extend(0.0).normalize_or_zero();
        let along_radius = normal.z.abs() < 1e-5 && normal.dot(radial) > 0.999;
        let along_axis = (normal.z.abs() - 1.0).abs() < 1e-5;
        assert!(
            along_radius || along_axis,
            "a tube is shaded along its radius on the side and its axis on an end, got {normal}"
        );
    }
    assert_eq!(
        smooth.normals.len(),
        12 * 2 * 2 + 2,
        "a rim corner once for the side and once for its end, a centre once"
    );
}

#[test]
fn faces_turning_past_the_crease_end_to_end_still_shade_as_one_when_each_edge_is_smooth() {
    // A six-sided double cone: neighbours round an apex turn 44 degrees, opposite faces 98.
    let cones = ball(Vec3::ZERO, 1.0, 2, 6);
    let smooth = smooth_normals(&cones, CREASE_DEG);

    let apex = normals_at(&smooth, 0);
    assert_eq!(apex.len(), 1, "the apex is one corner, got {apex:?}");
    assert!(
        apex[0].dot(Vec3::Z) > 0.9999,
        "shaded along the axis it stands on, got {}",
        apex[0]
    );
}

#[test]
fn an_edge_of_three_faces_is_kept_sharp() {
    let fin = Mesh::new(
        vec![
            Vec3::ZERO,
            Vec3::X,
            Vec3::new(0.5, 1.0, 0.0),
            Vec3::new(0.5, -1.0, 0.01),
            Vec3::new(0.5, 1.0, 0.02),
        ],
        vec![[0, 1, 2], [1, 0, 3], [0, 1, 4]],
    );
    let smooth = smooth_normals(&fin, CREASE_DEG);

    assert_eq!(
        normals_at(&smooth, 0).len(),
        3,
        "no two of three faces on one edge share a normal, however near they lie"
    );
}

#[test]
fn a_face_of_no_area_has_no_normal_and_bends_none() {
    let cube = cuboid(Vec3::ZERO, Vec3::ONE);
    let mut pinched = cube.clone();
    pinched.faces.push([0, 0, 1]);
    let plain = smooth_normals(&cube, CREASE_DEG);
    let smooth = smooth_normals(&pinched, CREASE_DEG);

    assert_eq!(
        smooth.corners[cube.faces.len() * 3..],
        [u32::MAX; 3],
        "a face of no area points at no normal"
    );
    assert_eq!(
        smooth.normals, plain.normals,
        "and none of the cube's leans"
    );
}
