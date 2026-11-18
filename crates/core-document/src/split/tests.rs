use core_edit::Edit;
use core_geometry::{Mesh, Scalar, Vec3, cuboid, signed_volume, transform_mesh};

use super::*;
use crate::apply::apply;
use crate::fixtures::{SIDE_MM, cube};

/// What f32 sums over a cube's faces drift by, mm³.
const VOLUME_MM3: Scalar = 1e-2;
const CUBE: ObjectId = ObjectId(1);

/// A plate whose one object is two cubes of [`SIDE_MM`], 5 mm apart, in one mesh.
fn two_cubes() -> Document {
    let mut project = cube();
    let mut mesh = cuboid(Vec3::ZERO, Vec3::splat(SIDE_MM));
    let beside = Vec3::new(SIDE_MM + 5.0, 0.0, 0.0);
    mesh.append(&cuboid(beside, beside + Vec3::splat(SIDE_MM)));
    project.models[0].source = std::sync::Arc::new(mesh);
    Document::from_project(project)
}

fn split_edit() -> Edit {
    Edit::Split {
        object: CUBE,
        smallest_mm3: None,
    }
}

fn placed(document: &Document, id: ObjectId) -> Mesh {
    let (state, meshes) = document.object(id).expect("an added object is there");
    transform_mesh(&meshes.source, state.transform)
}

#[test]
fn two_cubes_apart_split_into_two_parts_of_a_cube_each() {
    let mut document = two_cubes();

    let applied = apply(&mut document, &split_edit(), Cancel::never()).expect("it splits");

    assert_eq!(applied.removed, [CUBE]);
    let roles: Vec<Role> = applied.added.iter().map(|(_, role)| *role).collect();
    assert_eq!(roles, [Role::Part(1), Role::Part(2)]);
    for (id, _) in &applied.added {
        let volume = signed_volume(&placed(&document, *id));
        assert!(
            (volume - SIDE_MM.powi(3)).abs() < VOLUME_MM3,
            "each part holds 1000 mm³, got {volume}"
        );
    }
}

#[test]
fn one_cube_splits_into_nothing_new_and_keeps_its_id() {
    let mut document = Document::from_project(cube());

    let applied = apply(&mut document, &split_edit(), Cancel::never()).expect("it is whole");

    assert!(applied.removed.is_empty() && applied.added.is_empty());
    assert!(document.object(CUBE).is_some(), "the cube keeps its id");
}

#[test]
fn a_part_off_at_a_point_off_the_model_finds_no_neck() {
    let mut document = two_cubes();
    let edit = Edit::PartOff {
        object: CUBE,
        at_mm: Vec3::new(500.0, 500.0, 500.0),
    };

    let error = apply(&mut document, &edit, Cancel::never()).expect_err("nothing is there");

    assert_eq!(error, ApplyError::NoNeck(CUBE));
    assert!(document.object(CUBE).is_some());
}

#[test]
fn a_stopped_run_is_cancelled_and_leaves_the_document() {
    let mut document = two_cubes();
    let stopped = || true;

    for edit in [
        split_edit(),
        Edit::PartOff {
            object: CUBE,
            at_mm: Vec3::new(5.0, 5.0, 5.0),
        },
    ] {
        let error =
            apply(&mut document, &edit, Cancel::when(&stopped)).expect_err("the run is stopped");
        assert_eq!(error, ApplyError::Cancelled);
    }
    assert!(document.object(CUBE).is_some());
}

#[test]
fn a_part_off_of_an_object_with_no_faces_fails_with_the_geometry_s_reason() {
    let mut project = cube();
    project.models[0].source = std::sync::Arc::new(Mesh::default());
    let mut document = Document::from_project(project);
    let edit = Edit::PartOff {
        object: CUBE,
        at_mm: Vec3::ZERO,
    };

    let error = apply(&mut document, &edit, Cancel::never()).expect_err("there is nothing to cut");

    assert!(matches!(error, ApplyError::Failed { object, .. } if object == CUBE));
    assert!(document.object(CUBE).is_some());
}
