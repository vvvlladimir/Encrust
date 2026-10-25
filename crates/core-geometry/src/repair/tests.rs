use super::holes::fan;
use super::*;
use crate::{
    DEFAULT_WELD_TOLERANCE, Mesh, Scalar, Vec3, diagnose, orient_outward, signed_volume, weld,
};

/// A 2 mm cube, twelve triangles, wound outwards, standing on the origin.
fn cube() -> Mesh {
    let corner = |x: Scalar, y: Scalar, z: Scalar| Vec3::new(x, y, z) * 2.0;
    Mesh::new(
        vec![
            corner(0.0, 0.0, 0.0),
            corner(1.0, 0.0, 0.0),
            corner(1.0, 1.0, 0.0),
            corner(0.0, 1.0, 0.0),
            corner(0.0, 0.0, 1.0),
            corner(1.0, 0.0, 1.0),
            corner(1.0, 1.0, 1.0),
            corner(0.0, 1.0, 1.0),
        ],
        vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ],
    )
}

/// The cube with the two triangles of its top taken off, which is what a file written
/// by a broken exporter looks like: one square hole.
fn cube_missing_top() -> Mesh {
    let whole = cube();
    let faces = [&whole.faces[..2], &whole.faces[4..]].concat();
    Mesh::new(whole.vertices, faces)
}

/// The cube open at both ends: two square holes, two loops, one patch each.
fn cube_open_both_ends() -> Mesh {
    let whole = cube();
    Mesh::new(whole.vertices, whole.faces[4..].to_vec())
}

/// A fin standing on one edge of the cube, which is what a boolean leaves behind: the
/// edge is walked twice one way and once the other, and no winding covers all three.
fn cube_with_a_fin() -> Mesh {
    let mut mesh = cube();
    mesh.vertices.push(Vec3::new(1.0, 1.0, 4.0));
    mesh.faces.push([4, 5, 8]);
    mesh
}

#[test]
fn a_fin_is_closed_by_its_own_reverse_and_both_go() {
    let mut mesh = cube_with_a_fin();
    assert!(!diagnose(&mesh).is_closed(), "the fin opens the surface");

    let filled = fill_holes(&mut mesh);
    let dropped = remove_duplicate_faces(&mut mesh);

    assert_eq!(
        filled.loops_filled, 1,
        "the fin's foot and free edges are one loop"
    );
    assert_eq!(
        dropped.len(),
        2,
        "the fin and the patch over it enclose nothing"
    );
    assert_eq!(mesh.faces, cube().faces);
}

#[test]
fn a_face_laid_over_another_is_dropped_whichever_way_round_it_is() {
    let mut mesh = cube();
    let first = mesh.faces[0];
    mesh.faces.push(first);
    mesh.faces.push([first[0], first[2], first[1]]);
    assert_eq!(diagnose(&mesh).duplicate_faces, 2);

    assert_eq!(remove_duplicate_faces(&mut mesh), vec![12, 13]);
    assert_eq!(mesh.faces, cube().faces);
    assert!(diagnose(&mesh).is_closed(), "the edges are manifold again");
}

/// A wall of no thickness inside a solid: two faces back to back over one triangle.
/// Dropping only one would leave the other's edges walked twice the same way.
#[test]
fn a_pair_laid_back_to_back_goes_whole() {
    let mut mesh = cube();
    mesh.faces.push([0, 1, 6]);
    mesh.faces.push([0, 6, 1]);

    assert_eq!(remove_duplicate_faces(&mut mesh), vec![12, 13]);
    assert_eq!(mesh.faces, cube().faces);
}

#[test]
fn a_mesh_with_nothing_drawn_twice_keeps_every_face() {
    let mut mesh = cube();
    assert!(remove_duplicate_faces(&mut mesh).is_empty());
    assert_eq!(mesh.faces.len(), 12);
}

#[test]
fn a_closed_mesh_is_left_as_it_is() {
    let mut mesh = cube();
    let filled = fill_holes(&mut mesh);
    assert!(filled.is_empty());
    assert_eq!(mesh.faces.len(), 12);
    assert_eq!(mesh.vertices.len(), 8);
}

#[test]
fn a_square_hole_is_closed_with_two_triangles_and_no_new_vertex() {
    let mut mesh = cube_missing_top();
    let filled = fill_holes(&mut mesh);

    assert_eq!(filled.loops_filled, 1);
    assert_eq!(filled.faces_added, 2, "a square loop takes two triangles");
    assert_eq!(filled.loops_left, 0);
    assert_eq!(mesh.vertices.len(), 8, "a patch by area adds no vertex");
    assert!(diagnose(&mesh).is_closed());
}

#[test]
fn a_patch_is_wound_like_the_surface_it_closes() {
    let mut mesh = cube_missing_top();
    fill_holes(&mut mesh);
    // The cube is 2 mm on a side, so a patch wound inwards would read -8.
    assert!(
        (signed_volume(&mesh) - 8.0).abs() < 1e-4,
        "a closed 2 mm cube encloses 8 mm³, got {}",
        signed_volume(&mesh)
    );
    assert_eq!(orient_outward(&mut mesh).flipped_faces, 0);
}

#[test]
fn each_hole_of_a_mesh_open_at_both_ends_gets_its_own_patch() {
    let mut mesh = cube_open_both_ends();
    let filled = fill_holes(&mut mesh);

    assert_eq!(filled.loops_filled, 2);
    assert_eq!(filled.faces_added, 4);
    assert!(diagnose(&mesh).is_closed());
    assert!((signed_volume(&mesh) - 8.0).abs() < 1e-4);
}

/// The loop round a missing top and side bends through a right angle, so it has no
/// plane: the least-area patch is the two squares that were there.
#[test]
fn a_hole_round_a_corner_is_closed_with_what_was_there() {
    let whole = cube();
    let faces = [&whole.faces[..2], &whole.faces[4..6], &whole.faces[8..]].concat();
    let mut mesh = Mesh::new(whole.vertices, faces);
    let filled = fill_holes(&mut mesh);

    assert_eq!(filled.loops_filled, 1);
    assert_eq!(filled.faces_added, 4, "two squares, two triangles each");
    assert_eq!(mesh.vertices.len(), 8, "no fan, so no vertex of its own");
    assert!(diagnose(&mesh).is_sound());
    assert!(
        (signed_volume(&mesh) - 8.0).abs() < 1e-4,
        "a closed 2 mm cube encloses 8 mm³, got {}",
        signed_volume(&mesh)
    );
}

#[test]
fn a_loop_with_no_plane_is_left_open() {
    // Three vertices on one line: a loop with no plane to triangulate in, and a fan
    // from its middle would be as flat.
    let mut mesh = Mesh::new(
        vec![
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(3.0, 0.0, 0.0),
        ],
        vec![[0, 1, 2], [0, 2, 3]],
    );
    let before = mesh.faces.len();
    let filled = fill_holes(&mut mesh);
    assert_eq!(filled.loops_filled, 0);
    assert_eq!(filled.loops_left, 1, "the boundary has no patch");
    assert_eq!(mesh.faces.len(), before, "nothing is laid over a line");
}

#[test]
fn a_ring_no_triangulation_covers_is_fanned_from_a_vertex_of_its_own() {
    let mut mesh = Mesh::new(
        vec![
            Vec3::ZERO,
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(2.0, 2.0, 0.0),
            Vec3::new(0.0, 2.0, 0.0),
        ],
        Vec::new(),
    );
    let added = fan(&mut mesh, &[0, 1, 2, 3]);

    assert_eq!(added, 4, "one triangle per edge of the ring");
    assert_eq!(mesh.vertices.len(), 5, "the fan's own middle");
    assert_eq!(mesh.vertices[4], Vec3::new(1.0, 1.0, 0.0));
}

#[test]
fn filling_a_welded_mesh_closes_what_welding_left_open() {
    let unwelded = cube_missing_top();
    let mut welded = weld(&unwelded, DEFAULT_WELD_TOLERANCE).mesh;
    fill_holes(&mut welded);
    assert!(diagnose(&welded).is_closed());
}

#[test]
fn a_face_with_two_corners_on_one_point_is_collapsed_and_the_solid_stays_closed() {
    // A tetrahedron whose edge 0-1 was split at vertex 0 itself, by a vertex 4 standing
    // on it: the two faces either side of the split have no area.
    let mut split = Mesh::new(
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
    assert_eq!(diagnose(&split).degenerate_faces, 2);

    assert_eq!(collapse_flat_faces(&mut split), 2);
    let diagnostics = diagnose(&split);
    assert_eq!(diagnostics.degenerate_faces, 0, "got {diagnostics:?}");
    assert!(diagnostics.is_closed(), "got {diagnostics:?}");
    assert_eq!(split.faces.len(), 4, "the tetrahedron it was split from");
}

#[test]
fn corners_a_rounding_apart_are_one_point() {
    // The split again, its extra vertex a few bits of an f32 along the edge from
    // vertex 0, and a fin of two faces along that edge with no area between them.
    let mut split = Mesh::new(
        vec![
            Vec3::ZERO,
            Vec3::X,
            Vec3::Y,
            Vec3::Z,
            Vec3::new(1e-7, 0.0, 0.0),
        ],
        vec![
            [0, 2, 4],
            [4, 2, 1],
            [0, 4, 3],
            [4, 1, 3],
            [1, 2, 3],
            [2, 0, 3],
            [0, 4, 1],
            [0, 1, 4],
        ],
    );
    assert_eq!(diagnose(&split).degenerate_faces, 2);
    collapse_flat_faces(&mut split);
    let diagnostics = diagnose(&split);
    assert_eq!(diagnostics.degenerate_faces, 0, "got {diagnostics:?}");
    assert!(diagnostics.is_closed(), "got {diagnostics:?}");
}
