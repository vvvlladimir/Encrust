use super::*;
use crate::{Scalar, Vec3, ball, cuboid, signed_volume};

/// Corner tetrahedron with every face wound outwards. Volume is 1/6.
fn tetrahedron() -> Mesh {
    Mesh::new(
        vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z],
        vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
    )
}

#[test]
fn an_already_correct_mesh_is_left_alone() {
    let mut mesh = tetrahedron();
    let report = orient_outward(&mut mesh);

    assert!(report.unchanged());
    assert!(report.orientable);
    assert_eq!(report.inverted_shells, 0);
    assert!((signed_volume(&mesh) - 1.0 / 6.0).abs() < 1e-6);
}

#[test]
fn a_single_inverted_face_is_flipped_back() {
    let mut mesh = tetrahedron();
    mesh.faces[2].swap(1, 2);
    let report = orient_outward(&mut mesh);

    assert_eq!(report.flipped_faces, 1);
    assert_eq!(report.inverted_shells, 0);
    assert_eq!(mesh.faces, tetrahedron().faces);
}

#[test]
fn a_wholly_inverted_shell_is_turned_outwards() {
    let mut mesh = tetrahedron();
    for face in &mut mesh.faces {
        face.swap(1, 2);
    }
    let report = orient_outward(&mut mesh);

    assert_eq!(report.inverted_shells, 1);
    assert_eq!(report.flipped_faces, 4);
    assert!(signed_volume(&mesh) > 0.0);
}

#[test]
fn two_neighbouring_inverted_faces_are_turned_back_together() {
    let mut mesh = tetrahedron();
    mesh.faces[2].swap(1, 2);
    mesh.faces[3].swap(1, 2);
    let report = orient_outward(&mut mesh);

    assert!(report.orientable);
    assert_eq!(report.flipped_faces, 2);
    assert_eq!(mesh.faces, tetrahedron().faces);
}

/// A band of `segments` quads with a half twist, wound consistently everywhere but at
/// the seam where its ends meet turned over.
fn mobius_strip(segments: usize) -> Mesh {
    let mut vertices = Vec::new();
    for at in 0..segments {
        let around = std::f32::consts::TAU * at as f32 / segments as f32;
        let radial = Vec3::new(around.cos(), around.sin(), 0.0);
        let across = radial * (around / 2.0).cos() + Vec3::Z * (around / 2.0).sin();
        vertices.push(radial * 10.0 - across);
        vertices.push(radial * 10.0 + across);
    }
    let mut faces = Vec::new();
    for at in 0..segments as u32 {
        let (inner, outer) = (2 * at, 2 * at + 1);
        let (next_inner, next_outer) = if at + 1 == segments as u32 {
            (1, 0)
        } else {
            (2 * at + 2, 2 * at + 3)
        };
        faces.push([inner, outer, next_outer]);
        faces.push([inner, next_outer, next_inner]);
    }
    Mesh::new(vertices, faces)
}

/// No turning mends a half twist; a walk that tried would move the seam round the band
/// and turn half of it on the way.
#[test]
fn a_conflict_no_turning_can_mend_is_left_where_the_file_put_it() {
    let mut mesh = mobius_strip(12);
    let before = crate::diagnose(&mesh).unbalanced_edges;
    let report = orient_outward(&mut mesh);

    assert!(!report.orientable);
    assert!(report.unchanged(), "turned {} faces", report.flipped_faces);
    assert_eq!(crate::diagnose(&mesh).unbalanced_edges, before);
    assert_eq!(mesh.faces, mobius_strip(12).faces);
}

#[test]
fn an_empty_mesh_is_trivially_oriented() {
    let report = orient_outward(&mut Mesh::default());
    assert!(report.unchanged());
    assert!(report.orientable);
}

/// Cubes of the given sides centred on one point, each wound outward when its flag is
/// true. Sides 10, 6 and 2 enclose 1000, 216 and 8 mm³.
fn nested_cubes(cubes: &[(Scalar, bool)]) -> Mesh {
    let mut mesh = Mesh::default();
    for &(side, outward) in cubes {
        let mut cube = cuboid(Vec3::splat(-side / 2.0), Vec3::splat(side / 2.0));
        if !outward {
            cube.faces.iter_mut().for_each(|face| face.swap(1, 2));
        }
        mesh.append(&cube);
    }
    mesh
}

fn turned_inside_out(mut mesh: Mesh) -> Mesh {
    mesh.faces.iter_mut().for_each(|face| face.swap(1, 2));
    mesh
}

fn assert_volume(mesh: &Mesh, expected: Scalar) {
    let volume = signed_volume(mesh);
    assert!(
        (volume - expected).abs() < 1e-2,
        "expected {expected} mm³, got {volume}"
    );
}

#[test]
fn a_cavity_wound_inward_is_kept() {
    let mut mesh = nested_cubes(&[(10.0, true), (6.0, false)]);
    let report = orient_outward(&mut mesh);

    assert!(report.unchanged(), "turned {} faces", report.flipped_faces);
    assert_volume(&mesh, 1000.0 - 216.0);
}

#[test]
fn a_hollow_body_inside_out_is_turned_with_its_cavity() {
    let mut mesh = turned_inside_out(nested_cubes(&[(10.0, true), (6.0, false)]));
    let report = orient_outward(&mut mesh);

    assert_eq!(report.inverted_shells, 2);
    assert_volume(&mesh, 1000.0 - 216.0);
}

#[test]
fn a_body_inside_a_body_stays_a_body() {
    let mut mesh = nested_cubes(&[(10.0, true), (6.0, true)]);
    let report = orient_outward(&mut mesh);

    assert!(report.unchanged(), "turned {} faces", report.flipped_faces);
    assert_volume(&mesh, 1000.0 + 216.0);
}

#[test]
fn an_island_in_a_cavity_of_a_body_inside_out_is_turned_with_both() {
    let cubes = [(10.0, true), (6.0, false), (2.0, true)];
    let mut mesh = turned_inside_out(nested_cubes(&cubes));
    let report = orient_outward(&mut mesh);

    assert_eq!(report.inverted_shells, 3);
    assert_volume(&mesh, 1000.0 - 216.0 + 8.0);
}

#[test]
fn a_shell_inside_out_beside_another_is_turned_alone() {
    let mut mesh = cuboid(Vec3::ZERO, Vec3::splat(10.0));
    mesh.append(&turned_inside_out(cuboid(
        Vec3::new(20.0, 0.0, 0.0),
        Vec3::new(26.0, 6.0, 6.0),
    )));
    let report = orient_outward(&mut mesh);

    assert_eq!(report.inverted_shells, 1);
    assert_volume(&mesh, 1000.0 + 216.0);
}

/// The octahedron touches the cube's faces, so the sweep meets it first on the same x, and
/// rays from its face's centroid would graze the cube's diagonals.
#[test]
fn a_cavity_touching_its_holder_at_its_bounds_is_turned_with_it() {
    let mut mesh = turned_inside_out(ball(Vec3::ZERO, 5.0, 2, 4));
    mesh.append(&cuboid(Vec3::splat(-5.0), Vec3::splat(5.0)));
    let mut mesh = turned_inside_out(mesh);
    let report = orient_outward(&mut mesh);

    assert_eq!(report.inverted_shells, 2);
    assert_volume(&mesh, 1000.0 - 4.0 / 3.0 * 125.0);
}
