#[path = "shared/bodies.rs"]
mod bodies;

use core_geometry::{
    DEFAULT_WELD_TOLERANCE, Mesh, Vec3, diagnose, fill_holes, orient_outward, signed_volume, weld,
};

fn welded(mesh: &Mesh) -> Mesh {
    weld(mesh, DEFAULT_WELD_TOLERANCE).mesh
}

#[test]
fn welding_turns_an_stl_style_cube_into_a_closed_solid() {
    let raw = bodies::cube_unwelded(10.0);
    assert_eq!(raw.vertices.len(), 36, "STL repeats every vertex per face");
    assert_eq!(
        diagnose(&raw).boundary_edges,
        36,
        "unwelded, every edge looks open"
    );

    let result = weld(&raw, DEFAULT_WELD_TOLERANCE);
    let diagnostics = diagnose(&result.mesh);

    assert_eq!(result.mesh.vertices.len(), 8);
    assert_eq!(result.vertices_merged, 28);
    assert_eq!(result.faces_removed(), 0);
    assert!(diagnostics.is_closed());
    assert_eq!(diagnostics.euler_characteristic, 2);
}

#[test]
fn welding_absorbs_jitter_below_the_tolerance() {
    let mut raw = bodies::cube_unwelded(10.0);
    for (index, vertex) in raw.vertices.iter_mut().enumerate() {
        let nudge = if index % 2 == 0 { 2e-6 } else { -2e-6 };
        *vertex += Vec3::splat(nudge);
    }

    let diagnostics = diagnose(&welded(&raw));
    assert!(
        diagnostics.is_closed(),
        "jitter of 2e-6 mm must not leave the mesh open"
    );
    assert_eq!(diagnostics.vertices, 8);
}

#[test]
fn inverted_faces_are_flipped_back() {
    let mut mesh = bodies::cube_with_inverted_faces(10.0, 3);
    let report = orient_outward(&mut mesh);

    assert!(report.orientable);
    assert_eq!(report.flipped_faces, 3);
    assert_eq!(report.inverted_shells, 0);
    assert_eq!(mesh.faces, bodies::cube(10.0).faces);
    assert!((signed_volume(&mesh) - 1000.0).abs() < 1e-3);
}

#[test]
fn a_cube_wound_entirely_inwards_is_turned_outwards() {
    let mut mesh = bodies::cube_with_inverted_faces(10.0, 12);
    assert!(signed_volume(&mesh) < 0.0, "the fixture starts inside out");

    let report = orient_outward(&mut mesh);

    assert_eq!(report.inverted_shells, 1);
    assert_eq!(report.flipped_faces, 12);
    assert!((signed_volume(&mesh) - 1000.0).abs() < 1e-3);
}

#[test]
fn two_separate_solids_are_oriented_independently() {
    let mut mesh = bodies::cube(4.0);
    let offset = mesh.vertices.len() as u32;
    let mut second = bodies::tetrahedron();
    for vertex in &mut second.vertices {
        *vertex += Vec3::new(20.0, 0.0, 0.0);
    }
    mesh.vertices.extend(second.vertices);
    // Only the second solid is inverted.
    mesh.faces.extend(
        second
            .faces
            .iter()
            .map(|f| [f[0] + offset, f[2] + offset, f[1] + offset]),
    );

    assert_eq!(diagnose(&mesh).shells, 2);
    let report = orient_outward(&mut mesh);

    assert_eq!(report.inverted_shells, 1);
    assert_eq!(report.flipped_faces, 4, "only the tetrahedron moved");
    assert!(signed_volume(&mesh) > 0.0);
}

#[test]
fn a_mobius_strip_is_reported_as_non_orientable() {
    let mut mesh = bodies::mobius_strip(24);
    let diagnostics = diagnose(&mesh);

    assert_eq!(
        diagnostics.non_manifold_edges, 0,
        "the strip is still a manifold"
    );
    assert!(
        diagnostics.boundary_edges > 0,
        "the strip has one boundary loop"
    );

    let report = orient_outward(&mut mesh);
    assert!(
        !report.orientable,
        "no consistent winding exists for a Mobius strip"
    );
    assert_eq!(
        report.inverted_shells, 0,
        "an open shell has no inside to invert"
    );
}

#[test]
fn an_open_shell_is_never_inverted() {
    let mut mesh = bodies::open_box(4.0);
    for face in &mut mesh.faces {
        face.swap(1, 2);
    }
    let report = orient_outward(&mut mesh);

    assert!(report.orientable);
    assert_eq!(
        report.inverted_shells, 0,
        "outwards is undefined without a closed volume"
    );
    assert_eq!(report.flipped_faces, 0);
}

#[test]
fn filling_an_open_box_closes_it_at_the_volume_it_bounds() {
    let mut mesh = bodies::open_box(4.0);
    assert!(diagnose(&mesh).boundary_edges > 0, "the fixture is open");

    let filled = fill_holes(&mut mesh);
    let diagnostics = diagnose(&mesh);

    assert_eq!(filled.loops_filled, 1);
    assert_eq!(filled.loops_left, 0);
    assert!(diagnostics.is_closed());
    assert_eq!(diagnostics.euler_characteristic, 2);
    assert!(
        (signed_volume(&mesh) - 64.0).abs() < 1e-3,
        "a 4 mm cube holds 64 mm³, got {}",
        signed_volume(&mesh)
    );
}

#[test]
fn a_patched_sphere_with_a_hole_keeps_its_orientation() {
    let mut mesh = bodies::uv_sphere(5.0, 12, 16);
    let whole = signed_volume(&mesh);
    mesh.faces.drain(0..4);

    fill_holes(&mut mesh);
    let report = orient_outward(&mut mesh);

    assert!(report.orientable);
    assert_eq!(report.flipped_faces, 0, "the patch came out wound outwards");
    assert!(
        (signed_volume(&mesh) - whole).abs() < 0.5,
        "a patch over four faces of a sphere of radius 5 cannot move {whole} far"
    );
}

#[test]
fn a_mobius_strip_has_no_patch_to_close_it() {
    let mut mesh = bodies::mobius_strip(24);
    let before = mesh.faces.len();
    let filled = fill_holes(&mut mesh);

    assert!(
        filled.loops_filled + filled.loops_left > 0,
        "the strip has a boundary"
    );
    assert!(
        mesh.faces.len() >= before,
        "filling never removes a face of its own"
    );
}
