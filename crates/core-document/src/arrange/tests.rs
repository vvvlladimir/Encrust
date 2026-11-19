use std::sync::Arc;

use core_edit::{Edit, ObjectId};
use core_engine::project::ObjectSupportState;
use core_geometry::{Aabb, BoundingVolume, Mesh, Transform, Vec3, transform_mesh};
use core_volume::Cancel;

use super::*;
use crate::apply::apply;
use crate::fixtures::cube;

/// What a clearance of 2 mm may be short of, mm.
const GAP_TOL_MM: Scalar = 1e-3;

/// `count` cubes `side_mm` on a side, every one at the origin, ids from 1.
fn heap(count: u64, side_mm: Scalar) -> Document {
    let mut project = crate::fixtures::cube_of(side_mm);
    let (state, meshes) = (
        project.manifest.objects[0].clone(),
        project.models[0].clone(),
    );
    for id in 2..=count {
        project
            .manifest
            .objects
            .push(core_engine::project::ObjectState {
                id: ObjectId(id),
                ..state.clone()
            });
        project.models.push(meshes.clone());
    }
    Document::from_project(project)
}

fn arrange_on(document: &mut Document, plate_mm: Scalar) -> Applied {
    let edit = Edit::Arrange {
        plate_mm: [plate_mm, plate_mm],
        clearance_mm: 2.0,
    };
    apply(document, &edit, Cancel::never()).expect("a plate of cubes arranges")
}

fn bounds_of(document: &Document, id: u64) -> Aabb {
    let (state, meshes) = document.object(ObjectId(id)).expect("it is on the plate");
    transform_mesh(&meshes.source, state.transform)
        .aabb()
        .expect("a cube has faces")
}

fn apart_mm(a: &Aabb, b: &Aabb) -> Scalar {
    let x = (a.mins.x - b.maxs.x).max(b.mins.x - a.maxs.x);
    let y = (a.mins.y - b.maxs.y).max(b.mins.y - a.maxs.y);
    x.max(y)
}

#[test]
fn four_stacked_cubes_are_spread_clear_of_each_other_inside_the_plate() {
    let mut document = heap(4, 10.0);

    let applied = arrange_on(&mut document, 100.0);

    assert_eq!(
        applied.report,
        Report::Arranged {
            placed: 4,
            left_out: Vec::new()
        }
    );
    let all: Vec<Aabb> = (1..=4).map(|id| bounds_of(&document, id)).collect();
    for (at, one) in all.iter().enumerate() {
        assert!(
            one.mins.truncate().min_element() >= 0.0 && one.maxs.truncate().max_element() <= 100.0,
            "cube {at} stays on the 100 mm plate: {one:?}"
        );
        for other in &all[at + 1..] {
            let gap = apart_mm(one, other);
            assert!(gap >= 2.0 - GAP_TOL_MM, "cubes keep 2 mm apart, got {gap}");
        }
    }
}

#[test]
fn five_cubes_too_big_to_share_the_plate_leave_some_out_by_id() {
    let mut document = heap(5, 50.0);

    let applied = arrange_on(&mut document, 100.0);

    let Report::Arranged { placed, left_out } = applied.report else {
        panic!("an arrange reports what it placed");
    };
    assert!(!left_out.is_empty(), "five 50 mm cubes do not fit 100 mm");
    assert_eq!(placed + left_out.len(), 5);
    assert!(left_out.iter().all(|id| (1..=5).contains(&id.0)));
}

#[test]
fn a_hidden_object_is_left_where_it_stands() {
    let mut project = cube();
    project.manifest.objects[0].visible = false;
    let mut document = Document::from_project(project);

    let applied = arrange_on(&mut document, 100.0);

    assert_eq!(
        applied.report,
        Report::Arranged {
            placed: 0,
            left_out: Vec::new()
        }
    );
}

#[test]
fn an_object_with_no_surface_is_refused_by_id() {
    let mut project = cube();
    project.models[0].source = Arc::new(Mesh::default());
    project.manifest.objects[0].supports = ObjectSupportState::default();
    let mut document = Document::from_project(project);

    let edit = Edit::Arrange {
        plate_mm: [100.0, 100.0],
        clearance_mm: 2.0,
    };
    let error = apply(&mut document, &edit, Cancel::never()).expect_err("nothing to pack");

    assert!(
        matches!(error, ApplyError::NoFootprint(ObjectId(1))),
        "{error:?}"
    );
}

#[test]
fn a_cube_longer_than_the_plate_across_is_turned_onto_it() {
    let mut project = cube();
    project.models[0].source = Arc::new(core_geometry::cuboid(
        Vec3::ZERO,
        Vec3::new(120.0, 20.0, 10.0),
    ));
    let mut document = Document::from_project(project);

    let edit = Edit::Arrange {
        plate_mm: [100.0, 140.0],
        clearance_mm: 2.0,
    };
    apply(&mut document, &edit, Cancel::never()).expect("it can be turned to fit");

    let bounds = bounds_of(&document, 1);
    assert!(
        bounds.maxs.x - bounds.mins.x <= 100.0,
        "a 120 mm bar across a 100 mm plate is turned, got {bounds:?}"
    );
}

#[test]
fn lifted_cubes_keep_their_supports_clear_of_each_other() {
    let mut document = heap(2, 10.0);
    for id in 1..=2 {
        let lifted = document
            .object(ObjectId(id))
            .expect("on the plate")
            .0
            .transform;
        document.set_transform(ObjectId(id), crate::moved(lifted, Vec3::new(0.0, 0.0, 5.0)));
        let supports = Edit::Supports {
            object: ObjectId(id),
            profile: Some("medium".into()),
            at_mm: Vec::new(),
            only_at: false,
            cavity: false,
            shown_at: Vec::new(),
        };
        apply(&mut document, &supports, Cancel::never()).expect("a lifted cube is supported");
    }

    arrange_on(&mut document, 100.0);

    let boxes: Vec<Aabb> = (1..=2)
        .map(|id| {
            let cube = bounds_of(&document, id);
            let stood = crate::supports::standing_meshes(&document, ObjectId(id));
            assert!(!stood.is_empty(), "cube {id} stands on supports");
            stood
                .iter()
                .filter_map(|mesh| core_geometry::placed_aabb(mesh, Transform::default()))
                .fold(cube, |all, one| all.merged(&one))
        })
        .collect();
    let gap = apart_mm(&boxes[0], &boxes[1]);
    assert!(
        gap >= 2.0 - GAP_TOL_MM,
        "cubes and supports keep 2 mm apart within 1e-3, got {gap}"
    );
}
