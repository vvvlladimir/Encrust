use super::*;
use core_geometry::{Vec3, cuboid, cylinder, smooth_normals};

#[test]
fn a_box_lays_a_corner_once_for_each_face_meeting_there() {
    let cube = cuboid(Vec3::ZERO, Vec3::ONE);
    let smooth = smooth_normals(&cube, CREASE_DEG);
    let (vertices, indices) = smooth_vertices(&cube, &smooth, 0..cube.faces.len());

    assert_eq!(
        vertices.len(),
        24,
        "eight corners, each on three faces at right angles"
    );
    assert_eq!(indices.len(), cube.faces.len() * 3);
}

#[test]
fn a_part_of_a_mesh_lays_out_only_the_corners_its_faces_reach() {
    let tube = cylinder(Vec3::ZERO, Vec3::new(0.0, 0.0, 5.0), 1.0, 12);
    let smooth = smooth_normals(&tube, CREASE_DEG);
    let (vertices, indices) = smooth_vertices(&tube, &smooth, 0..2);

    assert_eq!(vertices.len(), 4, "one quad of the side has four corners");
    assert!(
        indices
            .iter()
            .all(|index| (*index as usize) < vertices.len()),
        "and every index points at one of them"
    );
}

#[test]
fn a_face_of_no_area_lays_no_triangle() {
    let mut pinched = cuboid(Vec3::ZERO, Vec3::ONE);
    pinched.faces.push([0, 0, 1]);
    let smooth = smooth_normals(&pinched, CREASE_DEG);
    let (vertices, indices) = smooth_vertices(&pinched, &smooth, 0..pinched.faces.len());

    assert_eq!(indices.len(), (pinched.faces.len() - 1) * 3);
    assert_eq!(vertices.len(), 24, "and no corner for it");
}
