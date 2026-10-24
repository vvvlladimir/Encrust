use super::*;
use core_geometry::{Quat, Vec3};

/// The face each index triple stands for, by its normal off its first corner.
fn led(vertices: &[ModelVertex], indices: &[u32]) -> Vec<[f32; 3]> {
    indices
        .chunks(3)
        .map(|face| vertices[face[0] as usize].normal)
        .collect()
}

#[test]
fn a_corner_two_faces_share_is_sent_once() {
    let square = Mesh::new(
        vec![Vec3::ZERO, Vec3::X, Vec3::new(1.0, 1.0, 0.0), Vec3::Y],
        vec![[0, 1, 2], [0, 2, 3]],
    );
    let (vertices, indices) = indexed_vertices(&square, 0..2);
    assert_eq!(vertices.len(), 4, "a square is four corners, not six");
    assert_ne!(
        indices[0], indices[3],
        "each face leads from a corner of its own"
    );
    assert_eq!(led(&vertices, &indices), [[0.0, 0.0, 1.0]; 2]);
}

/// A closed tetrahedron has four corners and four faces, each facing its own way, so
/// every face needs a leading corner and the corners just go round.
#[test]
fn every_face_of_a_closed_body_leads_with_its_own_normal() {
    let tetrahedron = Mesh::new(
        vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z],
        vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
    );
    let (vertices, indices) = indexed_vertices(&tetrahedron, 0..4);
    let third = 1.0 / 3.0_f32.sqrt();
    let expected = [
        [0.0, 0.0, -1.0],
        [0.0, -1.0, 0.0],
        [-1.0, 0.0, 0.0],
        [third, third, third],
    ];
    for (got, want) in led(&vertices, &indices).iter().zip(expected) {
        let off = got
            .iter()
            .zip(want)
            .map(|(g, w)| (g - w).abs())
            .fold(0.0, f32::max);
        assert!(
            off < 1e-6,
            "each face reads its own normal: {got:?} against {want:?}"
        );
    }
    assert!(
        vertices.len() <= 8,
        "corners are shared: {} for four",
        vertices.len()
    );
    for (face, original) in indices.chunks(3).zip(&tetrahedron.faces) {
        let got = [0, 1, 2].map(|k| vertices[face[k] as usize].position);
        let at = original.map(|v| tetrahedron.vertices[v as usize].to_array());
        let turns = [at, [at[1], at[2], at[0]], [at[2], at[0], at[1]]];
        assert!(
            turns.contains(&got),
            "a face is turned round, never flipped"
        );
    }
}

#[test]
fn a_face_led_last_ends_on_its_own_normal_and_keeps_its_winding() {
    let square = Mesh::new(
        vec![Vec3::ZERO, Vec3::X, Vec3::new(1.0, 1.0, 0.0), Vec3::Y],
        vec![[0, 1, 2], [0, 2, 3]],
    );
    let (vertices, mut indices) = indexed_vertices(&square, 0..2);
    let first = indices.clone();
    lead_last(&mut indices);
    for (turned, original) in indices.chunks(3).zip(first.chunks(3)) {
        assert_eq!(
            turned,
            [original[1], original[2], original[0]],
            "turned, not flipped"
        );
        assert_eq!(vertices[turned[2] as usize].normal, [0.0, 0.0, 1.0]);
    }
}

#[test]
fn a_range_is_numbered_from_its_own_corners() {
    let square = Mesh::new(
        vec![Vec3::ZERO, Vec3::X, Vec3::new(1.0, 1.0, 0.0), Vec3::Y],
        vec![[0, 1, 2], [0, 2, 3]],
    );
    let (vertices, indices) = indexed_vertices(&square, 1..2);
    assert_eq!(vertices.len(), 3);
    assert_eq!(indices, [0, 1, 2]);
    assert_eq!(vertices[2].position, [0.0, 1.0, 0.0]);
}

#[test]
fn a_degenerate_face_is_still_sent_facing_up() {
    let mesh = Mesh::new(vec![Vec3::ZERO, Vec3::X, Vec3::X], vec![[0, 1, 2]]);
    let (vertices, indices) = indexed_vertices(&mesh, 0..1);
    assert_eq!(indices.len(), 3);
    assert_eq!(led(&vertices, &indices), [[0.0, 0.0, 1.0]]);
}

#[test]
fn an_empty_mesh_produces_nothing() {
    let (vertices, indices) = indexed_vertices(&Mesh::default(), 0..0);
    assert!(vertices.is_empty() && indices.is_empty());
}

#[test]
fn a_non_uniform_scale_keeps_normals_perpendicular() {
    let transform = Transform {
        translation: Vec3::new(3.0, 0.0, 0.0),
        rotation: Quat::from_rotation_z(0.7),
        scale: Vec3::new(1.0, 1.0, 4.0),
        ..Transform::default()
    };
    let model = transform.to_matrix();
    let normals = normal_matrix(&model);

    // A 45 degree slope in the XZ plane: stretching Z by four steepens the surface,
    // so the transformed normal must tilt the other way to stay perpendicular.
    let along = Vec3::new(1.0, 0.0, 1.0).normalize();
    let normal = Vec3::new(-1.0, 0.0, 1.0).normalize();
    let moved_along = Mat3::from_mat4(model) * along;
    let moved_normal = normals * normal;
    assert!(moved_along.dot(moved_normal).abs() < 1e-5);
}

#[test]
fn a_flattened_object_falls_back_to_its_own_normals() {
    let flat = Transform {
        scale: Vec3::new(1.0, 1.0, 0.0),
        ..Transform::default()
    };
    assert_eq!(normal_matrix(&flat.to_matrix()), Mat3::IDENTITY);
}

#[test]
fn one_flipped_axis_mirrors_and_two_turn_the_model_round() {
    let scaled = |scale| {
        let transform = Transform {
            scale,
            ..Transform::default()
        };
        ModelInstance::new(transform, Color32::WHITE, NOT_MARKED).is_mirrored()
    };
    assert!(scaled(Vec3::new(-1.0, 1.0, 1.0)), "one flip is a mirror");
    assert!(
        !scaled(Vec3::new(-1.0, -1.0, 1.0)),
        "two flips are a half turn about the third axis"
    );
    assert!(
        !scaled(Vec3::new(2.0, 1.0, 0.5)),
        "a stretch mirrors nothing"
    );
}

#[test]
fn the_instance_stride_has_no_padding() {
    assert_eq!(
        size_of::<ModelInstance>(),
        (16 + 9 + 4 + 3) * size_of::<f32>()
    );
}
