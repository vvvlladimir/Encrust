use core_edit::Edit;
use core_geometry::{Scalar, Vec3, cuboid, signed_volume, transform_mesh};

use super::*;
use crate::apply::apply;
use crate::fixtures::{SIDE_MM, cube};

/// What f32 sums over a cube's faces drift by, mm³.
const VOLUME_MM3: Scalar = 1e-2;
const FIRST: ObjectId = ObjectId(1);
const SECOND: ObjectId = ObjectId(2);

/// Two cubes of [`SIDE_MM`], the second `gap_mm` along x of the first's far face; a negative
/// gap overlaps them.
fn two_cubes(gap_mm: Scalar) -> Document {
    let mut project = cube();
    let meshes = project.models[0].clone();
    let mut second = project.manifest.objects[0].clone();
    second.id = SECOND;
    second.transform.translation = Vec3::new(SIDE_MM + gap_mm, 0.0, 0.0);
    project.manifest.objects[0].id = FIRST;
    project.manifest.objects.push(second);
    project.models.push(meshes);
    Document::from_project(project)
}

fn combine_edit(op: Op, objects: Vec<ObjectId>) -> Edit {
    Edit::Combine {
        objects,
        op,
        toward: None,
    }
}

fn combined_volume(op: Op) -> Scalar {
    let mut document = two_cubes(-5.0);
    let edit = combine_edit(op, vec![FIRST, SECOND]);

    let applied = apply(&mut document, &edit, Cancel::never()).expect("the cubes overlap");

    assert_eq!(applied.removed, [FIRST, SECOND]);
    let [(id, Role::Combined)] = applied.added[..] else {
        panic!("one combined object, got {:?}", applied.added);
    };
    assert!(document.object(FIRST).is_none() && document.object(SECOND).is_none());
    let (state, meshes) = document.object(id).expect("the result is on the plate");
    signed_volume(&transform_mesh(&meshes.source, state.transform))
}

#[test]
fn cubes_overlapping_by_half_make_a_union_of_one_and_a_half_cubes() {
    let volume = combined_volume(Op::Union);
    assert!(
        (volume - 1500.0).abs() < VOLUME_MM3,
        "expected 1500, got {volume}"
    );
}

#[test]
fn cubes_overlapping_by_half_leave_half_a_cube_less_the_second() {
    let volume = combined_volume(Op::Difference);
    assert!(
        (volume - 500.0).abs() < VOLUME_MM3,
        "expected 500, got {volume}"
    );
}

#[test]
fn cubes_overlapping_by_half_intersect_in_half_a_cube() {
    let volume = combined_volume(Op::Intersection);
    assert!(
        (volume - 500.0).abs() < VOLUME_MM3,
        "expected 500, got {volume}"
    );
}

#[test]
fn cubes_apart_intersect_in_nothing_and_stay() {
    let mut document = two_cubes(5.0);
    let edit = combine_edit(Op::Intersection, vec![FIRST, SECOND]);

    let error = apply(&mut document, &edit, Cancel::never()).expect_err("they share nothing");

    assert_eq!(error, ApplyError::NothingLeft);
    assert!(document.object(FIRST).is_some() && document.object(SECOND).is_some());
}

#[test]
fn one_object_is_too_few_to_combine() {
    let mut document = two_cubes(5.0);
    for objects in [vec![FIRST], vec![FIRST, FIRST], Vec::new()] {
        let edit = combine_edit(Op::Union, objects);
        let error = apply(&mut document, &edit, Cancel::never()).expect_err("too few");
        assert_eq!(error, ApplyError::TooFew);
    }
}

#[test]
fn an_object_not_on_the_plate_is_named() {
    let mut document = two_cubes(5.0);
    let edit = combine_edit(Op::Union, vec![FIRST, ObjectId(9)]);

    let error = apply(&mut document, &edit, Cancel::never()).expect_err("no object 9");

    assert_eq!(error, ApplyError::NoObject(ObjectId(9)));
}

#[test]
fn a_stopped_run_is_cancelled_and_leaves_the_plate() {
    let mut document = two_cubes(-5.0);
    let stopped = || true;
    let edit = combine_edit(Op::Union, vec![FIRST, SECOND]);

    let error = apply(&mut document, &edit, Cancel::when(&stopped)).expect_err("stopped");

    assert_eq!(error, ApplyError::Cancelled);
    assert!(document.object(FIRST).is_some() && document.object(SECOND).is_some());
}

#[test]
fn a_sheet_is_not_a_solid_for_the_exact_cut() {
    let mut open = cuboid(Vec3::ZERO, Vec3::splat(SIDE_MM));
    let vertices = open.vertices.clone();
    open.faces
        .retain(|face| face.iter().all(|at| vertices[*at as usize].z == SIDE_MM));
    let error = combined(
        &[open, cuboid(Vec3::splat(5.0), Vec3::splat(20.0))],
        Boolean::Union,
        Some(Vec3::Z),
        Cancel::never(),
        FIRST,
    )
    .expect_err("the first is a sheet");

    assert_eq!(error, ApplyError::NotSolid(FIRST));
}
