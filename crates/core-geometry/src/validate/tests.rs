use super::*;
use crate::Vec3;

/// A cube of side `side` with every face cut into `n` by `n` squares, wound outward.
fn tessellated_cube(n: u32, side: Scalar) -> Mesh {
    let mut mesh = Mesh::default();
    let faces = [
        (Vec3::ZERO, Vec3::Y, Vec3::X),
        (Vec3::Z, Vec3::X, Vec3::Y),
        (Vec3::ZERO, Vec3::X, Vec3::Z),
        (Vec3::Y, Vec3::Z, Vec3::X),
        (Vec3::ZERO, Vec3::Z, Vec3::Y),
        (Vec3::X, Vec3::Y, Vec3::Z),
    ];
    for (origin, u, v) in faces {
        let base = mesh.vertices.len() as u32;
        for j in 0..=n {
            for i in 0..=n {
                let (s, t) = (i as Scalar / n as Scalar, j as Scalar / n as Scalar);
                mesh.vertices.push((origin + u * s + v * t) * side);
            }
        }
        for j in 0..n {
            for i in 0..n {
                let a = base + j * (n + 1) + i;
                let c = a + n + 1;
                mesh.faces.push([a, a + 1, c + 1]);
                mesh.faces.push([a, c + 1, c]);
            }
        }
    }
    mesh
}

#[test]
fn the_volume_of_a_dense_mesh_does_not_drift() {
    // 1.9 million faces, as a detailed resin model has; summed in f32 this came to
    // 1008.8 mm^3.
    let cube = tessellated_cube(400, 10.0);
    let volume = signed_volume(&cube);
    assert!(
        (volume - 1000.0).abs() < 0.1,
        "a 10 mm cube holds 1000 mm^3, got {volume}"
    );
}

fn two_triangles() -> Mesh {
    Mesh::new(
        vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::new(1.0, 1.0, 0.0)],
        vec![[0, 1, 2], [1, 3, 2]],
    )
}

/// An axis-aligned box from `mins` to `maxs`, twelve triangles wound outward.
fn box_mesh(mins: Vec3, maxs: Vec3) -> Mesh {
    crate::cuboid(mins, maxs)
}

#[test]
fn a_box_has_its_centre_of_mass_in_the_middle() {
    let mesh = box_mesh(Vec3::new(10.0, -4.0, 0.0), Vec3::new(14.0, 2.0, 8.0));
    let centre = center_of_mass(&mesh).expect("a box has vertices");
    // A solid box is symmetric about all three midplanes.
    assert!(
        centre.abs_diff_eq(Vec3::new(12.0, -1.0, 4.0), 1e-4),
        "expected the middle of the box, got {centre:?}"
    );
}

#[test]
fn a_tetrahedron_has_its_centre_of_mass_at_the_average_of_its_corners() {
    let mesh = Mesh::new(
        vec![
            Vec3::ZERO,
            Vec3::new(3.0, 0.0, 0.0),
            Vec3::new(0.0, 3.0, 0.0),
            Vec3::new(0.0, 0.0, 3.0),
        ],
        vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
    );
    let centre = center_of_mass(&mesh).expect("a tetrahedron has vertices");
    // The centroid of a solid tetrahedron is the mean of its four vertices.
    assert!(
        centre.abs_diff_eq(Vec3::splat(0.75), 1e-5),
        "expected (0.75, 0.75, 0.75), got {centre:?}"
    );
}

#[test]
fn a_flat_sheet_falls_back_to_the_middle_of_its_bounding_box() {
    let centre = center_of_mass(&two_triangles()).expect("the sheet has vertices");
    assert!(
        centre.abs_diff_eq(Vec3::new(0.5, 0.5, 0.0), 1e-6),
        "a sheet encloses nothing, so the box decides; got {centre:?}"
    );
}

#[test]
fn a_mesh_with_no_vertices_has_no_centre_of_mass() {
    assert_eq!(center_of_mass(&Mesh::new(Vec::new(), Vec::new())), None);
}

#[test]
fn the_centre_of_mass_is_not_the_middle_of_the_bounding_box() {
    // Two boxes of the same 2 mm side, one at the origin, one eight times further
    // out: mass is shared evenly, so the centroid sits between them, not in the
    // middle of the pair's bounding box.
    let mut mesh = box_mesh(Vec3::ZERO, Vec3::splat(2.0));
    let far = box_mesh(Vec3::new(16.0, 0.0, 0.0), Vec3::new(18.0, 2.0, 2.0));
    let offset = mesh.vertices.len() as u32;
    mesh.vertices.extend(far.vertices);
    mesh.faces
        .extend(far.faces.iter().map(|f| f.map(|i| i + offset)));

    let centre = center_of_mass(&mesh).expect("two boxes have vertices");
    assert!(
        (centre.x - 9.0).abs() < 1e-3,
        "equal masses at x = 1 and x = 17 average to 9, got {centre:?}"
    );
}

#[test]
fn an_open_sheet_reports_its_boundary() {
    let diagnostics = diagnose(&two_triangles());
    assert_eq!(diagnostics.boundary_edges, 4);
    assert_eq!(diagnostics.non_manifold_edges, 0);
    assert_eq!(diagnostics.shells, 1);
    assert!(!diagnostics.is_closed());
}

/// Two boxes meeting along one vertical edge: the four faces at it pair off, two
/// walking it each way, so each box still has an inside.
#[test]
fn two_boxes_sharing_an_edge_branch_there_and_still_close() {
    let mut mesh = box_mesh(Vec3::ZERO, Vec3::splat(2.0));
    let other = box_mesh(Vec3::new(2.0, 2.0, 0.0), Vec3::new(4.0, 4.0, 2.0));
    let offset = mesh.vertices.len() as u32;
    mesh.vertices.extend(other.vertices);
    mesh.faces
        .extend(other.faces.iter().map(|f| f.map(|i| i + offset)));
    let mesh = crate::weld(&mesh, crate::DEFAULT_WELD_TOLERANCE).mesh;

    let diagnostics = diagnose(&mesh);
    assert_eq!(
        diagnostics.non_manifold_edges, 1,
        "the shared edge branches"
    );
    assert_eq!(diagnostics.unbalanced_edges, 0);
    assert!(diagnostics.is_closed(), "two solids, no hole in either");
}

/// One face turned round leaves its three edges walked twice the same way, which is
/// where a surface stops having one inside.
#[test]
fn a_face_turned_round_leaves_its_edges_unbalanced() {
    let mut mesh = box_mesh(Vec3::ZERO, Vec3::splat(2.0));
    mesh.faces[0].swap(1, 2);

    let diagnostics = diagnose(&mesh);
    assert_eq!(diagnostics.boundary_edges, 0);
    assert_eq!(diagnostics.non_manifold_edges, 0);
    assert_eq!(diagnostics.unbalanced_edges, 3);
    assert!(!diagnostics.is_closed());
    assert!(
        diagnostics.is_sound(),
        "the fill rule takes what the box encloses, so it slices as it stands"
    );
}

#[test]
fn a_hole_makes_a_mesh_unsound() {
    let mut mesh = box_mesh(Vec3::ZERO, Vec3::splat(2.0));
    mesh.faces.pop();
    assert!(!diagnose(&mesh).is_sound());
}

#[test]
fn a_third_face_on_one_edge_is_non_manifold() {
    let mut mesh = two_triangles();
    mesh.vertices.push(Vec3::new(0.0, 0.0, 1.0));
    mesh.faces.push([1, 2, 4]);
    assert_eq!(diagnose(&mesh).non_manifold_edges, 1);
}

#[test]
fn a_repeated_face_is_counted_once_as_a_duplicate() {
    let mut mesh = two_triangles();
    mesh.faces.push([2, 0, 1]);
    assert_eq!(diagnose(&mesh).duplicate_faces, 1);
}

/// Wound opposite ways the two enclose nothing between them, so both are counted.
#[test]
fn a_face_laid_back_to_back_on_another_counts_as_a_pair() {
    let mut mesh = two_triangles();
    mesh.faces.push([0, 2, 1]);
    assert_eq!(duplicate_faces(&mesh), vec![0, 2]);
}

#[test]
fn a_collinear_face_is_degenerate() {
    let mesh = Mesh::new(
        vec![Vec3::ZERO, Vec3::X, Vec3::new(2.0, 0.0, 0.0)],
        vec![[0, 1, 2]],
    );
    assert_eq!(diagnose(&mesh).degenerate_faces, 1);
}

#[test]
fn a_vertex_no_face_uses_is_reported() {
    let mut mesh = two_triangles();
    mesh.vertices.push(Vec3::new(9.0, 9.0, 9.0));
    assert_eq!(diagnose(&mesh).unreferenced_vertices, 1);
}

#[test]
fn an_empty_mesh_has_no_shells() {
    let diagnostics = diagnose(&Mesh::default());
    assert_eq!(diagnostics.shells, 0);
    assert_eq!(diagnostics.euler_characteristic, 0);
}
