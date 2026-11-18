use super::*;
use core_geometry::{Bvh, closest_point};

/// A cube from the origin to `(size, size, size)`, wound outward.
fn cube(size: Scalar) -> Mesh {
    core_geometry::cuboid(Vec3::ZERO, Vec3::splat(size))
}

fn inside_by(mesh: &Mesh, mode: SignMode, point: Vec3) -> bool {
    let signer = Signer::new(mesh, mode);
    let found = closest_point(mesh, point).expect("the mesh has faces");
    signer.is_inside(mesh, point, &found)
}

#[test]
fn the_pseudonormal_signs_a_closed_cube() {
    let mesh = cube(10.0);
    assert!(inside_by(&mesh, SignMode::Pseudonormal, Vec3::splat(5.0)));
    assert!(!inside_by(
        &mesh,
        SignMode::Pseudonormal,
        Vec3::new(-2.0, 5.0, 5.0)
    ));
}

/// A point past a corner is nearest to the corner itself, where only the angle
/// weighting gives the outward direction the faces disagree about.
#[test]
fn a_point_past_a_corner_is_outside() {
    let mesh = cube(10.0);
    assert!(!inside_by(
        &mesh,
        SignMode::Pseudonormal,
        Vec3::new(-1.0, -1.0, -1.0)
    ));
    assert!(!inside_by(
        &mesh,
        SignMode::Pseudonormal,
        Vec3::new(11.0, 11.0, 5.0)
    ));
}

#[test]
fn both_modes_agree_on_a_closed_mesh() {
    let mesh = cube(10.0);
    let bvh = Bvh::build(&mesh);
    for point in [
        Vec3::splat(5.0),
        Vec3::new(0.5, 5.0, 5.0),
        Vec3::new(-0.5, 5.0, 5.0),
        Vec3::new(9.5, 9.5, 9.5),
        Vec3::new(10.5, 5.0, 5.0),
    ] {
        let found = bvh.closest(point).expect("the cube has faces");
        let by_normal = Signer::new(&mesh, SignMode::Pseudonormal);
        let by_winding = Signer::new(&mesh, SignMode::Winding);
        assert_eq!(
            by_normal.is_inside(&mesh, point, &found),
            by_winding.is_inside(&mesh, point, &found),
            "the two modes disagree at {point}"
        );
    }
}

/// A sliver 24 mm long and a tenth wide lying face down, and a face rising from its
/// short edge. Its barycentrics lose that edge to cancellation, which signed the point
/// off it by the sliver's own normal and called it inside.
#[test]
fn a_point_off_the_short_edge_of_a_sliver_is_signed_by_the_edge() {
    let mesh = Mesh::new(
        vec![
            Vec3::new(97.21413, 97.78362, 0.0),
            Vec3::new(97.08101, 122.19565, 0.0),
            Vec3::new(97.09066, 121.70752, 0.0),
            Vec3::new(96.0, 122.0, 1.5),
        ],
        vec![[0, 1, 2], [2, 1, 3]],
    );
    let point = Vec3::new(100.0, 122.2, 1.0);
    let found = closest_point(&mesh, point).expect("the mesh has faces");

    assert!(
        !Signer::new(&mesh, SignMode::Pseudonormal).is_inside(&mesh, point, &found),
        "the point stands off the ridge, outside both faces"
    );
}

/// A sliver so thin that `f32` puts its determinant below zero, which read as a face
/// with no area and signed the point by the sliver's own normal.
#[test]
fn a_sliver_whose_determinant_cancels_below_zero_is_still_signed_by_its_edge() {
    let mesh = Mesh::new(
        vec![
            Vec3::new(97.21413, 97.78362, 0.0),
            Vec3::new(97.074_936, 122.76993, 0.0),
            Vec3::new(97.078_255, 122.29129, 0.0),
            Vec3::new(96.0, 122.5, 1.5),
        ],
        vec![[0, 1, 2], [2, 1, 3]],
    );
    let point = Vec3::new(100.0, 122.6, 1.0);
    let found = closest_point(&mesh, point).expect("the mesh has faces");

    assert!(
        !Signer::new(&mesh, SignMode::Pseudonormal).is_inside(&mesh, point, &found),
        "the point stands off the ridge, outside both faces"
    );
}

/// A sheet with no thickness standing out of a face, as a model carries a stray film:
/// its two sides cancel along its rim, where the pseudonormal is noise.
#[test]
fn a_closed_mesh_of_two_shells_is_signed_by_the_winding_number() {
    let mut mesh = cube(10.0);
    mesh.append(&core_geometry::cuboid(
        Vec3::new(5.0, 2.0, 8.0),
        Vec3::new(5.0, 8.0, 14.0),
    ));
    assert!(diagnose(&mesh).is_closed(), "the sheet is walked both ways");
    assert!(matches!(
        Signer::new(&mesh, SignMode::Auto),
        Signer::Winding(_)
    ));
}

/// The shells `outer_shells` leaves out, a film and a body inside another, change no
/// point's side, so the whole mesh's winding number signs what is left.
#[test]
fn the_whole_mesh_signs_its_bounding_shells_as_they_sign_themselves() {
    let surface = cube(10.0);
    let mut whole = surface.clone();
    whole.append(&core_geometry::cuboid(
        Vec3::new(5.0, 2.0, 8.0),
        Vec3::new(5.0, 8.0, 14.0),
    ));
    whole.append(&core_geometry::cuboid(Vec3::splat(3.0), Vec3::splat(4.0)));
    let winding = Winding::build(&whole);
    let shared = Signer::of_shells(&surface, SignMode::Auto, (&whole, &winding));
    let own = Signer::new(&surface, SignMode::Winding);
    for point in [
        Vec3::splat(3.5),
        Vec3::splat(5.0),
        Vec3::new(5.0, 5.0, 9.5),
        Vec3::new(5.2, 5.0, 12.0),
        Vec3::new(-1.0, 5.0, 5.0),
    ] {
        let found = closest_point(&surface, point).expect("the cube has faces");
        assert_eq!(
            shared.is_inside(&surface, point, &found),
            own.is_inside(&surface, point, &found),
            "the two disagree at {point}"
        );
    }
}

/// Two cubes welded along one edge: one closed shell, but four faces meet on that edge and
/// its pseudonormal is not defined there.
#[test]
fn one_closed_shell_with_a_branching_edge_is_signed_by_the_winding_number() {
    let mut pair = cube(10.0);
    pair.append(&core_geometry::cuboid(
        Vec3::new(10.0, 10.0, 0.0),
        Vec3::new(20.0, 20.0, 10.0),
    ));
    let welded = core_geometry::weld(&pair, 1e-4).mesh;
    let found = diagnose(&welded);
    assert_eq!((found.shells, found.non_manifold_edges), (1, 1));
    assert!(matches!(
        Signer::new(&welded, SignMode::Auto),
        Signer::Winding(_)
    ));
}

#[test]
fn a_mesh_with_a_hole_falls_back_to_the_winding_number() {
    let mut mesh = cube(10.0);
    mesh.faces.truncate(10);
    assert!(matches!(
        Signer::new(&mesh, SignMode::Auto),
        Signer::Winding(_)
    ));
    assert!(matches!(
        Signer::new(&cube(10.0), SignMode::Auto),
        Signer::Pseudonormal(_)
    ));
}
