use super::*;
use crate::fixtures::cube_with_a_finer_top;
use crate::{cuboid, diagnose, signed_volume};

#[test]
fn a_finer_side_stitches_into_a_closed_cube() {
    let mut mesh = cube_with_a_finer_top(2, 0.0);
    assert!(diagnose(&mesh).boundary_edges > 0, "the rim is open");

    let stitched = stitch_seams(&mut mesh, DEFAULT_SEAM_TOLERANCE);

    assert_eq!(stitched.edges_split, 4, "one rim edge on each side");
    assert_eq!(stitched.vertices_inserted, 4);
    let found = diagnose(&mesh);
    assert!(found.is_sound(), "got {found:?}");
    let volume = signed_volume(&mesh);
    assert!((volume - 1.0).abs() < 1e-6, "expected 1 mm^3, got {volume}");
}

#[test]
fn stitching_makes_no_vertex() {
    let mut mesh = cube_with_a_finer_top(3, 0.0);
    let vertices = mesh.vertices.clone();

    let stitched = stitch_seams(&mut mesh, DEFAULT_SEAM_TOLERANCE);

    assert_eq!(stitched.vertices_inserted, 8, "two on each rim edge");
    assert_eq!(mesh.vertices, vertices);
    assert!(diagnose(&mesh).is_sound(), "got {:?}", diagnose(&mesh));
}

#[test]
fn a_vertex_standing_off_the_edge_past_the_tolerance_is_left() {
    let mut mesh = cube_with_a_finer_top(2, 0.1);
    let faces = mesh.faces.clone();

    let stitched = stitch_seams(&mut mesh, DEFAULT_SEAM_TOLERANCE);

    assert_eq!(stitched, Stitched::default());
    assert_eq!(mesh.faces, faces);
}

#[test]
fn a_vertex_inside_the_tolerance_is_stitched() {
    let mut mesh = cube_with_a_finer_top(2, 0.005);

    let stitched = stitch_seams(&mut mesh, DEFAULT_SEAM_TOLERANCE);

    assert_eq!(stitched.edges_split, 4);
    assert_eq!(diagnose(&mesh).boundary_edges, 0);
}

#[test]
fn a_closed_mesh_has_no_seam() {
    let mut mesh = cuboid(Vec3::ZERO, Vec3::ONE);
    assert_eq!(
        stitch_seams(&mut mesh, DEFAULT_SEAM_TOLERANCE),
        Stitched::default()
    );
}

#[test]
fn an_open_edge_only_touching_another_is_not_a_seam() {
    // A sheet standing on the middle of another's open edge, across it rather than along.
    let mut mesh = Mesh::new(
        vec![
            Vec3::ZERO,
            Vec3::X,
            Vec3::new(0.5, 1.0, 0.0),
            Vec3::new(0.5, 0.0, 0.0),
            Vec3::new(0.5, -1.0, 1.0),
            Vec3::new(0.5, 1.0, 1.0),
        ],
        vec![[0, 1, 2], [3, 4, 5]],
    );

    assert_eq!(
        stitch_seams(&mut mesh, DEFAULT_SEAM_TOLERANCE),
        Stitched::default()
    );
}
