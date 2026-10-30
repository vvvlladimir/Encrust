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
fn wrapping_keeps_a_cavity_wound_inward() {
    let outer = ball(Vec3::ZERO, 10.0, 32, 64);
    let inner = ball(Vec3::ZERO, 5.0, 32, 64);
    let expected = signed_volume(&outer) - signed_volume(&inner);
    let wrapped = wrap(
        &joined(outer, &inverted(&inner)),
        RepairSettings::default(),
        Cancel::never(),
    )
    .expect("a hollow ball reads exactly");

    assert!(wrapped.wrapped);
    let volume = signed_volume(&wrapped.mesh);
    assert!(
        (volume - expected).abs() < 1e-3 * expected,
        "expected {expected} mm^3, the shell between the balls, got {volume}"
    );
    assert_eq!(diagnose(&wrapped.mesh).shells, 2);
}

#[test]
fn wrapping_swallows_a_ball_inside_a_ball() {
    let outer = ball(Vec3::ZERO, 10.0, 32, 64);
    let inner = ball(Vec3::ZERO, 5.0, 32, 64);
    let expected = signed_volume(&outer);
    let wrapped = wrap(
        &joined(outer, &inner),
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
fn a_surface_crossing_itself_is_left_as_the_repair_made_it() {
    let mesh = cube_through_itself();
    let wrapped =
        wrap(&mesh, RepairSettings::default(), Cancel::never()).expect("the cube repairs");

    assert!(
        !wrapped.wrapped,
        "the engine returns a shell crossing itself still crossing"
    );
    assert_eq!(
        wrapped.mesh, mesh,
        "nothing to repair, so the cube stands as it came"
    );
}

#[test]
fn deep_repair_leaves_the_wrapped_model_sound() {
    let (one, other) = crossing_cubes();
    let deep = repair(
        &joined(one, &inverted(&other)),
        RepairMethod::Deep,
        RepairSettings::default(),
        Cancel::never(),
    )
    .expect("the cubes wrap");

    assert!(
        diagnose(&deep.mesh).is_sound(),
        "got {:?}",
        diagnose(&deep.mesh)
    );
    assert!(!deep.mended.kept_as_found);
}

#[test]
fn deep_repair_reports_the_hole_its_first_pass_closed() {
    let (one, mut other) = crossing_cubes();
    other.faces.pop();
    let deep = repair(
        &joined(one, &other),
        RepairMethod::Deep,
        RepairSettings::default(),
        Cancel::never(),
    )
    .expect("the cubes wrap");

    assert_eq!(
        deep.mended.filled.loops_filled, 1,
        "one face was missing, so one hole was closed before the wrap"
    );
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
fn an_open_surface_is_left_as_the_repair_made_it() {
    let sheet = Mesh::new(
        vec![Vec3::ZERO, Vec3::X * 10.0, Vec3::Y * 10.0],
        vec![[0, 1, 2]],
    );
    let wrapped = wrap(&sheet, RepairSettings::default(), Cancel::never())
        .expect("the repair stands where nothing can be unioned");

    assert!(!wrapped.wrapped);
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
fn one_sound_shell_crossing_nothing_is_wrapped_as_it_is() {
    let cube = cuboid(Vec3::ZERO, Vec3::splat(10.0));
    let wrapped = wrap(&cube, RepairSettings::default(), Cancel::never()).expect("a cube wraps");

    assert!(wrapped.wrapped);
    assert_eq!(
        wrapped.mesh, cube,
        "nothing to union, so the repaired cube stands"
    );
}

#[test]
fn a_shell_crossing_itself_is_told_from_shells_crossing_each_other() {
    let crossings_of = |mesh: &Mesh| self_intersections(mesh, &Bvh::build(mesh));
    let through = cube_through_itself();
    let (one, other) = crossing_cubes();
    let pair = joined(one, &other);

    assert!(crosses_itself(
        &list_shells(&through),
        &crossings_of(&through)
    ));
    assert!(
        !crosses_itself(&list_shells(&pair), &crossings_of(&pair)),
        "two cubes cross each other, and neither itself"
    );
}

#[test]
fn a_wrap_that_unions_its_shells_reports_up_to_the_finished_mesh() {
    let (one, other) = crossing_cubes();
    let progress = crate::Progress::default();
    let wrapped = wrap(
        &joined(one, &other),
        RepairSettings::default(),
        Cancel::never().reporting(&progress),
    )
    .expect("two closed cubes union exactly");

    assert!(wrapped.wrapped);
    assert_eq!(progress.read(), Some((Step::Finishing, 0, 0)));
}

#[test]
fn a_repair_on_the_mesh_alone_reports_mending() {
    let progress = crate::Progress::default();
    repair(
        &cuboid(Vec3::ZERO, Vec3::splat(10.0)),
        RepairMethod::default(),
        RepairSettings::default(),
        Cancel::never().reporting(&progress),
    )
    .expect("a closed cube repairs");

    assert_eq!(progress.read(), Some((Step::Mending, 0, 0)));
}
