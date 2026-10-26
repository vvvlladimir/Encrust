use super::*;
use core_geometry::{Vec3, ball, cuboid, signed_volume};

/// Two meshes as one.
fn joined(one: Mesh, other: &Mesh) -> Mesh {
    let mut mesh = one;
    mesh.append(other);
    mesh
}

/// `mesh` turned inside out.
fn inverted(mesh: &Mesh) -> Mesh {
    let mut mesh = mesh.clone();
    for face in &mut mesh.faces {
        face.swap(1, 2);
    }
    mesh
}

/// A 10 mm cube, and one like it pushed 5, 3 and 4 mm into it: they share 5 × 7 × 6 mm.
fn crossing_cubes() -> (Mesh, Mesh) {
    let shift = Vec3::new(5.0, 3.0, 4.0);
    (
        cuboid(Vec3::ZERO, Vec3::splat(10.0)),
        cuboid(shift, shift + Vec3::splat(10.0)),
    )
}

#[test]
fn wrapping_two_crossing_cubes_is_their_union() {
    let (one, other) = crossing_cubes();
    let wrapped = wrap(
        &joined(one, &other),
        RepairSettings::default(),
        Cancel::never(),
    )
    .expect("two closed cubes union exactly");

    assert_eq!(wrapped.voxel_mm, None, "cut exactly");
    assert!(wrapped.wrapped);
    let found = diagnose(&wrapped.mesh);
    assert!(found.is_sound() && found.shells == 1, "got {found:?}");
    assert!(self_intersections(&wrapped.mesh, &Bvh::build(&wrapped.mesh)).is_empty());
    let volume = signed_volume(&wrapped.mesh);
    assert!(
        (volume - 1790.0).abs() < 1e-2,
        "expected 2000 - 210 mm^3, got {volume}"
    );
}

#[test]
fn wrapping_swallows_a_ball_inside_a_ball() {
    // Repair turns every closed shell outward (ADR 0269), so the inner ball is a body and
    // the outer surface holds it.
    let outer = ball(Vec3::ZERO, 10.0, 32, 64);
    let inner = ball(Vec3::ZERO, 5.0, 32, 64);
    let expected = signed_volume(&outer);
    let wrapped = wrap(
        &joined(outer, &inverted(&inner)),
        RepairSettings::default(),
        Cancel::never(),
    )
    .expect("two closed balls union exactly");

    let volume = signed_volume(&wrapped.mesh);
    assert!(
        (volume - expected).abs() < 1e-3 * expected,
        "expected {expected} mm^3, the outer ball's, got {volume}"
    );
    assert_eq!(diagnose(&wrapped.mesh).shells, 1);
}

/// The 10 mm cube with its far top corner pulled down through its floor: three of its faces
/// cross the floor, and the engine reads it without undoing that.
fn cube_through_itself() -> Mesh {
    let mut mesh = cuboid(Vec3::ZERO, Vec3::splat(10.0));
    let far = mesh
        .vertices
        .iter()
        .position(|corner| corner.min_element() > 5.0)
        .expect("the cube has a far corner");
    mesh.vertices[far] = Vec3::new(7.0, 6.0, -5.0);
    mesh
}

#[test]
fn a_surface_crossing_itself_is_wrapped_through_the_field() {
    let mesh = cube_through_itself();
    assert!(!self_intersections(&mesh, &Bvh::build(&mesh)).is_empty());

    let wrapped = wrap(&mesh, RepairSettings::default(), Cancel::never())
        .expect("the field reads what the engine leaves crossing");

    assert_eq!(wrapped.voxel_mm, Some(DEFAULT_WRAP_MM));
    let found = diagnose(&wrapped.mesh);
    assert!(found.is_sound() && found.shells == 1, "got {found:?}");
    assert!(self_intersections(&wrapped.mesh, &Bvh::build(&wrapped.mesh)).is_empty());
}

#[test]
fn deep_repair_leaves_the_wrapped_model_sound() {
    let deep = repair(
        &cube_through_itself(),
        RepairMethod::Deep,
        RepairSettings::default(),
        Cancel::never(),
    )
    .expect("the cube wraps");

    assert!(
        diagnose(&deep.mesh).is_sound(),
        "got {:?}",
        diagnose(&deep.mesh)
    );
    assert!(!deep.mended.kept_as_found);
}

#[test]
fn an_empty_mesh_has_nothing_to_wrap() {
    let refused = wrap(&Mesh::default(), RepairSettings::default(), Cancel::never());
    assert!(
        matches!(refused, Err(VolumeError::EmptyMesh)),
        "got {refused:?}"
    );
}

#[test]
fn a_lone_sheet_wraps_into_nothing() {
    let sheet = Mesh::new(
        vec![Vec3::ZERO, Vec3::X * 10.0, Vec3::Y * 10.0],
        vec![[0, 1, 2]],
    );
    let refused = wrap(&sheet, RepairSettings::default(), Cancel::never());
    assert!(
        matches!(refused, Err(VolumeError::NothingLeft)),
        "got {refused:?}"
    );
}

#[test]
fn a_stopped_wrap_says_so() {
    let (one, other) = crossing_cubes();
    let stop = || true;
    let stopped = wrap(
        &joined(one, &inverted(&other)),
        RepairSettings::default(),
        Cancel::when(&stop),
    );
    assert!(
        matches!(stopped, Err(VolumeError::Cancelled)),
        "got {stopped:?}"
    );
}

#[test]
fn a_surface_from_the_field_is_taken_only_when_it_is_better() {
    let crossing = cube_through_itself();
    let clean = cuboid(Vec3::ZERO, Vec3::splat(10.0));
    let mut open = clean.clone();
    open.faces.pop();

    assert!(better(&clean, &crossing), "sound and crossing nothing");
    assert!(!better(&open, &crossing), "a hole is worse than a crossing");
    assert!(
        !better(&crossing, &crossing),
        "crossing as much is no better"
    );
}
