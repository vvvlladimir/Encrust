use core_edit::Edit;
use core_geometry::{signed_volume, transform_mesh};

use super::*;
use crate::apply::apply;
use crate::fixtures::{SIDE_MM, cube, cubes_with_plug};

const CUBE: ObjectId = ObjectId(1);

fn bounds_of(document: &Document, id: ObjectId) -> core_geometry::Aabb {
    let (state, meshes) = document.object(id).expect("the object is on the plate");
    transform_mesh(&meshes.source, state.transform)
        .aabb()
        .expect("a cube has faces")
}

#[test]
fn a_deleted_cube_leaves_an_empty_plate() {
    let mut document = Document::from_project(cube());

    let applied = apply(&mut document, &Edit::Delete { object: CUBE }).expect("it is there");

    assert_eq!(applied.removed, [CUBE]);
    assert!(document.into_project().manifest.objects.is_empty());
}

#[test]
fn deleting_an_object_not_on_the_plate_is_refused() {
    let mut document = Document::from_project(cube());

    let error = apply(
        &mut document,
        &Edit::Delete {
            object: ObjectId(9),
        },
    )
    .expect_err("there is no object 9");

    assert_eq!(error, ApplyError::NoObject(ObjectId(9)));
}

#[test]
fn deleting_a_cube_takes_its_plugs_with_it_and_keeps_the_others() {
    let mut document = Document::from_project(cubes_with_plug());

    let applied = apply(
        &mut document,
        &Edit::Delete {
            object: ObjectId(2),
        },
    )
    .expect("it is there");

    assert_eq!(applied.removed, [ObjectId(2), ObjectId(3)]);
    let ids: Vec<ObjectId> = (document.into_project().manifest.objects.iter())
        .map(|object| object.id)
        .collect();
    assert_eq!(ids, [CUBE]);
}

#[test]
fn a_cube_duplicated_by_twenty_stands_beside_it_under_an_id_above_every_other() {
    let mut document = Document::from_project(cube());
    let edit = Edit::Duplicate {
        object: CUBE,
        offset_mm: Vec3::new(20.0, 0.0, 0.0),
    };

    let applied = apply(&mut document, &edit).expect("a cube copies");

    let [(copy, role)] = applied.added[..] else {
        panic!("one copy, got {:?}", applied.added);
    };
    assert_eq!(role, Role::Copy);
    assert!(
        copy > CUBE,
        "the copy's id is above the cube's, got {copy:?}"
    );
    let bounds = bounds_of(&document, copy);
    assert!(
        (bounds.mins.x - 20.0).abs() < 1e-4 && (bounds.maxs.x - 30.0).abs() < 1e-4,
        "the copy spans x 20 to 30, got {} to {}",
        bounds.mins.x,
        bounds.maxs.x
    );
    assert_eq!(bounds_of(&document, CUBE).mins.x, 0.0, "the cube stays");
    assert_eq!(document.into_project().manifest.objects.len(), 2);
}

#[test]
fn a_copy_beside_a_plugged_cube_leaves_the_plug_naming_that_cube() {
    let mut document = Document::from_project(cubes_with_plug());
    let edit = Edit::Duplicate {
        object: ObjectId(2),
        offset_mm: Vec3::X,
    };

    apply(&mut document, &edit).expect("a cube copies");

    let objects = document.into_project().manifest.objects;
    let plugs: Vec<Option<u32>> = objects.iter().map(|object| object.plug_of).collect();
    assert_eq!(
        plugs,
        [None, None, None, Some(1)],
        "the plug still names the second cube"
    );
}

#[test]
fn a_cube_mirrored_across_x_keeps_its_bounds_and_its_volume() {
    let mut document = Document::from_project(cube());

    let applied = apply(
        &mut document,
        &Edit::Mirror {
            object: CUBE,
            axis: Axis::X,
        },
    )
    .expect("a cube mirrors");

    assert_eq!(applied.report, Report::Placed);
    let bounds = bounds_of(&document, CUBE);
    assert!(
        (bounds.mins.to_array().iter().all(|v| v.abs() < 1e-4))
            && ((bounds.maxs - Vec3::splat(SIDE_MM)).abs().max_element() < 1e-4),
        "the cube still spans 0 to 10 on every axis, got {bounds:?}"
    );
    let (state, meshes) = document.object(CUBE).expect("still there");
    let volume = signed_volume(&transform_mesh(&meshes.source, state.transform));
    assert!(
        state.transform.scale.x < 0.0,
        "the cube is flipped, its X scale -1, got {}",
        state.transform.scale.x
    );
    assert!(
        (volume - 1000.0).abs() < 0.01,
        "the mirrored cube holds 1000 mm³, got {volume}"
    );
}

#[test]
fn mirroring_an_object_not_on_the_plate_is_refused() {
    let mut document = Document::from_project(cube());
    let edit = Edit::Mirror {
        object: ObjectId(9),
        axis: Axis::Z,
    };

    let error = apply(&mut document, &edit).expect_err("there is no object 9");

    assert_eq!(error, ApplyError::NoObject(ObjectId(9)));
}

#[test]
fn duplicating_an_object_not_on_the_plate_is_refused() {
    let mut document = Document::from_project(cube());
    let edit = Edit::Duplicate {
        object: ObjectId(9),
        offset_mm: Vec3::X,
    };

    let error = apply(&mut document, &edit).expect_err("there is no object 9");

    assert_eq!(error, ApplyError::NoObject(ObjectId(9)));
}
