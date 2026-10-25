use super::*;
use crate::{DEFAULT_WELD_TOLERANCE, Transform, Vec3, cuboid, signed_volume, weld};

/// The unit cube standing on the origin, wound outwards.
fn unit_cube() -> Mesh {
    cuboid(Vec3::ZERO, Vec3::ONE)
}

/// Whether `face` lies in the top of [`unit_cube`], whose corners stand at 0 or 1.
fn on_top(mesh: &Mesh, face: [u32; 3]) -> bool {
    face.iter()
        .all(|&corner| mesh.vertices[corner as usize].z > 0.5)
}

/// The unit cube with the two triangles of its top taken off: one square hole.
fn open_cube() -> Mesh {
    let mut mesh = unit_cube();
    let whole = mesh.clone();
    mesh.faces.retain(|&face| !on_top(&whole, face));
    mesh
}

#[test]
fn an_open_cube_comes_back_closed_with_its_volume() {
    let mut mesh = open_cube();
    let mended = mend(&mut mesh);

    assert_eq!(mended.filled.loops_filled, 1);
    assert!(!mended.kept_as_found);
    assert!(diagnose(&mesh).is_sound());
    let volume = signed_volume(&mesh);
    assert!((volume - 1.0).abs() < 1e-6, "expected 1 mm^3, got {volume}");
}

#[test]
fn a_face_drawn_twice_beside_a_hole_comes_back_sound() {
    // What a broken exporter leaves: one side of the cube missing, and a triangle next
    // to it written twice, which makes three edges look as if they branched.
    let mut mesh = open_cube();
    let beside = *mesh
        .faces
        .iter()
        .find(|face| {
            face.iter()
                .any(|&corner| mesh.vertices[corner as usize].z > 0.5)
        })
        .expect("a wall reaches the open top");
    mesh.faces.push(beside);
    assert!(!diagnose(&mesh).is_sound());

    let mended = mend(&mut mesh);

    assert_eq!(mended.duplicates_removed, 1);
    assert_eq!(mended.filled.loops_filled, 1);
    assert!(diagnose(&mesh).is_sound(), "got {:?}", diagnose(&mesh));
}

#[test]
fn a_seam_two_solids_share_is_not_something_to_repair() {
    // The open cube and a closed one meeting along one vertical edge: four faces use it,
    // twice each way, which is a seam rather than a hole.
    let mut mesh = open_cube();
    let other = crate::transform_mesh(
        &unit_cube(),
        Transform::from_translation(Vec3::new(1.0, 1.0, 0.0)),
    );
    let offset = mesh.vertices.len() as u32;
    mesh.vertices.extend(other.vertices);
    mesh.faces.extend(
        other
            .faces
            .iter()
            .map(|face| face.map(|index| index + offset)),
    );
    let mut mesh = weld(&mesh, DEFAULT_WELD_TOLERANCE).mesh;
    assert_eq!(diagnose(&mesh).non_manifold_edges, 1, "the shared edge");

    let mended = mend(&mut mesh);

    assert_eq!(mended.duplicates_removed, 0, "a seam costs no face");
    assert!(!mended.kept_as_found);
    assert_eq!(mended.filled.loops_filled, 1);
    assert!(diagnose(&mesh).is_sound(), "got {:?}", diagnose(&mesh));
}

#[test]
fn a_fin_is_cancelled_by_its_patch_and_the_model_comes_back_as_it_was() {
    // A fin standing on one edge of the cube, which is what a boolean leaves behind:
    // no winding covers the three faces that edge now has.
    let mut mesh = unit_cube();
    let top = *mesh
        .faces
        .iter()
        .find(|&&face| on_top(&mesh, face))
        .expect("the cube has a top");
    let tip = mesh.vertices.len() as u32;
    mesh.vertices.push(Vec3::new(0.5, 0.5, 2.0));
    mesh.faces.push([top[0], top[1], tip]);

    let mended = mend(&mut mesh);

    assert_eq!(mended.duplicates_removed, 2, "the fin and its reverse");
    assert_eq!(mesh.faces, unit_cube().faces);
}

#[test]
fn a_model_that_is_one_sheet_is_kept_rather_than_cancelled_away() {
    let mut mesh = Mesh::new(vec![Vec3::ZERO, Vec3::X, Vec3::Y], vec![[0, 1, 2]]);
    let mended = mend(&mut mesh);

    assert!(mended.kept_as_found);
    assert_eq!(mesh.faces, vec![[0, 1, 2]]);
}

#[test]
fn a_face_with_no_area_between_two_corners_on_one_point_is_repaired() {
    // A tetrahedron with its edge 0-1 split at vertex 0 by a vertex standing on it.
    let mut mesh = Mesh::new(
        vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z, Vec3::ZERO],
        vec![
            [0, 2, 4],
            [4, 2, 1],
            [0, 4, 3],
            [4, 1, 3],
            [1, 2, 3],
            [2, 0, 3],
        ],
    );
    let mended = mend(&mut mesh);

    assert!(!mended.kept_as_found);
    assert_eq!(wrong(&diagnose(&mesh)), 0, "got {:?}", diagnose(&mesh));
}
