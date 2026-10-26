use super::*;
use crate::scene::{ImportSummary, Imported};
use core_geometry::{
    Mended, Mesh, MeshDiagnostics, Orientation, Transform, Vec3, cuboid, diagnose,
};

/// What the fields a test does not set read as: a plain repair, not wrapped.
fn mended_alone() -> Repaired {
    Repaired {
        id: ObjectId::for_test(0),
        name: String::new(),
        shaped: crate::scene::Shaped::of(Arc::new(Mesh::default())),
        mended: Mended::default(),
        method: RepairMethod::default(),
        voxel_mm: None,
        wrapped: false,
    }
}

fn settle(repairs: &mut Repairs, scene: &mut Scene) -> Status {
    let mut status = Status::default();
    while repairs.poll(scene, &mut status) {
        std::thread::yield_now();
    }
    status
}

fn object(scene: &mut Scene, boundary_edges: usize) -> ObjectId {
    let mesh = Mesh::new(vec![Vec3::ZERO, Vec3::X, Vec3::Y], vec![[0, 1, 2]]);
    let mesh = Arc::new(mesh);
    scene.insert(Imported::new(
        "model.stl".to_owned(),
        Arc::clone(&mesh),
        Transform::default(),
        ImportSummary {
            vertices_merged: 0,
            faces_removed: 0,
            orientation: Orientation {
                flipped_faces: 0,
                inverted_shells: 0,
                orientable: true,
            },
            diagnostics: MeshDiagnostics {
                boundary_edges,
                unbalanced_edges: boundary_edges,
                ..diagnose(&mesh)
            },
        },
    ))
}

#[test]
fn repair_is_offered_for_a_broken_model_and_not_a_sound_one() {
    let mut scene = Scene::default();
    let broken = object(&mut scene, 3);
    object(&mut scene, 0);
    let repairs = Repairs::default();

    let offered: Vec<ObjectId> = repairs.offered(&scene).map(|object| object.id).collect();
    assert_eq!(offered, vec![broken]);
}

#[test]
fn a_model_kept_as_it_is_is_no_longer_offered() {
    let mut scene = Scene::default();
    let id = object(&mut scene, 3);
    let mut repairs = Repairs::default();
    repairs.keep_as_it_is(id);

    assert_eq!(repairs.offered(&scene).count(), 0);
    assert_eq!(broken_on_the_plate(&scene), 1, "it is still broken");
}

#[test]
fn a_repair_that_could_not_mend_a_model_is_not_offered_again() {
    let mut scene = Scene::default();
    // A lone triangle would be closed by its own reverse and cancel away, so it is kept.
    let id = object(&mut scene, 3);
    let mut repairs = Repairs::default();
    repairs.start(&scene, id, RepairState::default(), &mut Status::default());
    while repairs.poll(&mut scene, &mut Status::default()) {
        std::thread::yield_now();
    }

    assert!(!scene.get(id).expect("still there").summary.is_sound());
    assert_eq!(repairs.offered(&scene).count(), 0);
}

#[test]
fn a_model_under_repair_is_no_longer_offered() {
    let mut scene = Scene::default();
    let id = open_box(&mut scene);
    let mut repairs = Repairs::default();
    repairs.start(&scene, id, RepairState::default(), &mut Status::default());

    assert_eq!(repairs.offered(&scene).count(), 0);
}

#[test]
fn a_broken_model_on_the_plate_is_counted_before_slicing() {
    let mut scene = Scene::default();
    object(&mut scene, 3);
    object(&mut scene, 0);

    assert_eq!(broken_on_the_plate(&scene), 1);
}

/// A box with the two triangles of its top missing, diagnosed for real, which is a
/// model the window calls broken.
fn open_box(scene: &mut Scene) -> ObjectId {
    let mesh = Arc::new(Mesh::new(
        vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, 1.0),
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(0.0, 1.0, 1.0),
        ],
        vec![
            [0, 2, 1],
            [0, 3, 2],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ],
    ));
    scene.insert(Imported::new(
        "open.stl".to_owned(),
        Arc::clone(&mesh),
        Transform::default(),
        ImportSummary {
            vertices_merged: 0,
            faces_removed: 0,
            orientation: Orientation {
                flipped_faces: 0,
                inverted_shells: 0,
                orientable: true,
            },
            diagnostics: diagnose(&mesh),
        },
    ))
}

#[test]
fn a_model_that_came_back_closed_is_no_longer_called_broken() {
    let mut scene = Scene::default();
    let id = open_box(&mut scene);
    assert_eq!(broken_on_the_plate(&scene), 1);

    let mut mesh = (*scene.get(id).expect("the box is there").mesh).clone();
    let filled = core_geometry::fill_holes(&mut mesh);
    let orientation = core_geometry::orient_outward(&mut mesh);
    let status = apply(
        &mut scene,
        &Repaired {
            id,
            name: "open.stl".to_owned(),
            shaped: crate::scene::Shaped::of(Arc::new(mesh)),
            mended: Mended {
                stitched: core_geometry::Stitched::default(),
                shells_dropped: 0,
                filled,
                duplicates_removed: 0,
                orientation: Some(orientation),
                kept_as_found: false,
            },
            ..mended_alone()
        },
    );

    assert!(!status.is_error());
    let object = scene.get(id).expect("the box is still there");
    assert!(object.summary.is_sound());
    assert!(object.summary.defects().is_empty(), "the mark goes with it");
    assert_eq!(broken_on_the_plate(&scene), 0);
}

#[test]
fn a_repair_of_a_model_that_has_been_deleted_is_reported_rather_than_applied() {
    let mut scene = Scene::default();
    let id = object(&mut scene, 3);
    let mesh = Arc::clone(&scene.get(id).expect("inserted").mesh);
    scene.remove(id);

    let status = apply(
        &mut scene,
        &Repaired {
            id,
            name: "model.stl".to_owned(),
            shaped: crate::scene::Shaped::of(mesh),
            mended: Mended {
                stitched: core_geometry::Stitched::default(),
                shells_dropped: 0,
                filled: core_geometry::Filled::default(),
                duplicates_removed: 0,
                orientation: None,
                kept_as_found: false,
            },
            ..mended_alone()
        },
    );
    assert!(status.is_error());
}

#[test]
fn a_repair_that_would_worsen_the_model_leaves_it_as_it_came() {
    let mut scene = Scene::default();
    let id = open_box(&mut scene);
    let before = Arc::clone(&scene.get(id).expect("the box is there").mesh);

    let status = apply(
        &mut scene,
        &Repaired {
            id,
            name: "open.stl".to_owned(),
            shaped: crate::scene::Shaped::of(Arc::new(Mesh::default())),
            mended: Mended {
                stitched: core_geometry::Stitched::default(),
                shells_dropped: 0,
                filled: core_geometry::Filled::default(),
                duplicates_removed: 0,
                orientation: None,
                kept_as_found: true,
            },
            ..mended_alone()
        },
    );

    assert!(status.is_error(), "the user is told nothing was changed");
    let object = scene.get(id).expect("the box is still there");
    assert!(
        Arc::ptr_eq(&object.mesh, &before),
        "the mesh is the one it had"
    );
}

#[test]
fn a_repair_that_only_stitched_seams_says_so() {
    let mut scene = Scene::default();
    let id = open_box(&mut scene);
    let mesh = Arc::clone(&scene.get(id).expect("the box is there").mesh);

    let status = apply(
        &mut scene,
        &Repaired {
            id,
            name: "seamed.stl".to_owned(),
            shaped: crate::scene::Shaped::of(mesh),
            mended: Mended {
                stitched: core_geometry::Stitched {
                    edges_split: 4,
                    vertices_inserted: 4,
                },
                shells_dropped: 0,
                filled: core_geometry::Filled::default(),
                duplicates_removed: 0,
                orientation: None,
                kept_as_found: false,
            },
            ..mended_alone()
        },
    );

    assert!(!status.is_error());
    assert!(
        status.text().contains("stitched 4 seams"),
        "got {}",
        status.text()
    );
}

#[test]
fn a_wrap_stands_even_where_the_repair_under_it_was_kept_as_found() {
    let mut scene = Scene::default();
    let id = open_box(&mut scene);
    let wrapped = Arc::new(cuboid(Vec3::ZERO, Vec3::ONE));

    let status = apply(
        &mut scene,
        &Repaired {
            id,
            name: "open.stl".to_owned(),
            shaped: crate::scene::Shaped::of(Arc::clone(&wrapped)),
            mended: Mended {
                kept_as_found: true,
                ..Mended::default()
            },
            method: RepairMethod::Wrap,
            voxel_mm: Some(0.2),
            wrapped: true,
        },
    );

    assert!(!status.is_error(), "{status:?}");
    assert!(
        status.text().contains("0.20 mm lattice"),
        "got {}",
        status.text()
    );
    let object = scene.get(id).expect("the box is still there");
    assert!(
        Arc::ptr_eq(&object.mesh, &wrapped),
        "the wrap's surface stands"
    );
}

#[test]
fn a_wrap_the_field_would_have_worsened_says_so() {
    let mut scene = Scene::default();
    let id = open_box(&mut scene);
    let mesh = Arc::clone(&scene.get(id).expect("the box is there").mesh);

    let status = apply(
        &mut scene,
        &Repaired {
            id,
            name: "open.stl".to_owned(),
            shaped: crate::scene::Shaped::of(mesh),
            method: RepairMethod::Wrap,
            ..mended_alone()
        },
    );

    assert_eq!(
        status,
        Status::Info(tr!(
            "repair-done",
            name = "open.stl",
            what = tr!("repair-not-wrapped")
        ))
    );
}

#[test]
fn close_small_holes_leaves_a_hole_of_two_faces_open() {
    let mut scene = Scene::default();
    let id = open_box(&mut scene);
    let mut repairs = Repairs::default();
    let state = RepairState {
        method: RepairMethod::Mend(MendMethod::CloseSmallHoles),
        ..RepairState::default()
    };
    repairs.start(&scene, id, state, &mut Status::default());
    settle(&mut repairs, &mut scene);

    let object = scene.get(id).expect("the box is still there");
    assert_eq!(
        object.summary.diagnostics.holes, 1,
        "the square hole is open"
    );
}

#[test]
fn a_cancelled_repair_leaves_the_model_as_it_was() {
    let mut scene = Scene::default();
    let id = open_box(&mut scene);
    let before = Arc::clone(&scene.get(id).expect("the box is there").mesh);
    let mut repairs = Repairs::default();
    repairs.start(&scene, id, RepairState::default(), &mut Status::default());

    let mut status = Status::default();
    repairs.cancel(&mut status);

    assert_eq!(status, Status::Info(tr!("repair-cancelled")));
    assert!(!repairs.is_busy());
    assert!(!settle(&mut repairs, &mut scene).is_error());
    let object = scene.get(id).expect("the box is still there");
    assert!(
        Arc::ptr_eq(&object.mesh, &before),
        "the mesh is the one it had"
    );
}

#[test]
fn two_crossing_cubes_count_their_crossings_until_the_mesh_changes() {
    let mut scene = Scene::default();
    // Pushed in off the diagonals, so no edge of one lies along a face's own seam.
    let shift = Vec3::new(0.5, 0.3, 0.4);
    let mut crossing = cuboid(Vec3::ZERO, Vec3::ONE);
    crossing.append(&cuboid(shift, shift + Vec3::ONE));
    let id = scene.insert(Imported::new(
        "crossing.stl".to_owned(),
        Arc::new(crossing.clone()),
        Transform::default(),
        ImportSummary::made(diagnose(&crossing)),
    ));
    let mut repairs = Repairs::default();
    let object = scene.get(id).expect("inserted");
    assert_eq!(repairs.crossings_of(object), None, "nothing counted yet");

    repairs.count_crossings(&scene, &[id]);
    settle(&mut repairs, &mut scene);
    let object = scene.get(id).expect("still there");
    let counted = repairs.crossings_of(object).flatten();
    assert!(counted.is_some_and(|pairs| pairs > 0), "got {counted:?}");

    let reshaped = crate::scene::Shaped::of(Arc::new(cuboid(Vec3::ZERO, Vec3::ONE)));
    scene.get_mut(id).expect("still there").reshape(reshaped);
    settle(&mut repairs, &mut scene);
    let object = scene.get(id).expect("still there");
    assert_eq!(
        repairs.crossings_of(object),
        None,
        "the count was of the old mesh"
    );
}
