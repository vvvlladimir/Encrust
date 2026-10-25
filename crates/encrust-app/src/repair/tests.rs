use super::*;
use crate::scene::{ImportSummary, Imported};
use core_geometry::{Mended, Mesh, MeshDiagnostics, Orientation, Transform, Vec3, diagnose};

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
    repairs.start(&scene, id, &mut Status::default());
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
    repairs.start(&scene, id, &mut Status::default());

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
        },
    );

    assert!(!status.is_error());
    assert!(
        status.text().contains("stitched 4 seams"),
        "got {}",
        status.text()
    );
}
