use core_geometry::{Vec3, cuboid, diagnose};

use super::*;
use crate::scene::{ImportSummary, Imported};

/// A 10 mm cube whose underside hangs 5 mm over the plate, with one support under it.
fn supported_cube() -> Scene {
    let cube = cuboid(Vec3::splat(-5.0), Vec3::splat(5.0));
    let summary = ImportSummary::made(diagnose(&cube));
    let mut scene = Scene::default();
    let transform = Transform::from_translation(Vec3::new(0.0, 0.0, 10.0));
    let id = scene.insert(Imported::new(
        "cube".to_owned(),
        Arc::new(cube),
        transform,
        summary,
    ));
    let object = scene.get_mut(id).expect("just inserted");
    object.supports.add(Vec3::new(0.0, 0.0, 5.0), transform, 0);
    scene
}

fn settle(growth: &mut SupportGrowth, scene: &mut Scene) {
    while growth.poll(scene, &mut Status::Idle) {
        std::thread::yield_now();
    }
}

#[test]
fn grown_trees_land_on_their_model_and_build_without_growing_again() {
    let mut scene = supported_cube();
    let table = [SupportProfile::shipped("medium").expect("the medium preset ships")];
    let mut growth = SupportGrowth::default();

    growth.start(&scene.objects()[0], &table);
    settle(&mut growth, &mut scene);

    let object = &mut scene.objects_mut()[0];
    assert!(!object.supports.needs_growing(object.transform, &table));
    object.refresh_supports(&table);
    assert_eq!(object.supports.standing(), 1, "the one support stands");
}

#[test]
fn a_model_moved_while_its_trees_grow_holds_its_supports_pending() {
    let mut scene = supported_cube();
    let table = [SupportProfile::shipped("medium").expect("the medium preset ships")];
    let mut growth = SupportGrowth::default();
    growth.start(&scene.objects()[0], &table);
    settle(&mut growth, &mut scene);
    let object = &mut scene.objects_mut()[0];
    object.refresh_supports(&table);
    assert!(!object.supports_pending());

    object.transform = Transform::from_translation(Vec3::new(20.0, 0.0, 10.0));
    assert!(object.supports.outgrown(object.transform, &table));
    assert!(
        object.supports_pending(),
        "what stood where the model was is not sliced"
    );
}

#[test]
fn columns_carried_while_their_trees_regrow_are_drilled_where_they_were_built() {
    let mut scene = supported_cube();
    let table = [SupportProfile::shipped("medium").expect("the medium preset ships")];
    let mut growth = SupportGrowth::default();
    growth.start(&scene.objects()[0], &table);
    settle(&mut growth, &mut scene);
    let object = &mut scene.objects_mut()[0];
    object.refresh_supports(&table);
    let built_at = object.transform;

    object.transform = Transform::from_translation(Vec3::new(20.0, 0.0, 10.0));
    assert!(object.supports.outgrown(object.transform, &table));
    assert_eq!(object.supports_under().transform, built_at);
    assert!(
        object.supports_built().is_none(),
        "nothing built elsewhere is picked or drilled"
    );
}

#[test]
fn a_painted_patch_moves_with_its_model_while_the_trees_grow_again() {
    let mut doc = Doc {
        scene: supported_cube(),
        ..Doc::default()
    };
    let mut tool = SupportTool::default();
    let object = &mut doc.scene.objects_mut()[0];
    let model = Arc::clone(&object.mesh);
    let adjacency = core_geometry::Adjacency::of(&model);
    object
        .supports
        .paint_face(&model, &adjacency, 0, 30.0, false, true);
    refresh_supports(&mut doc, &mut tool, false);
    settle(&mut doc.growth, &mut doc.scene);
    refresh_supports(&mut doc, &mut tool, false);
    let was = patch_bounds(&doc.scene);

    doc.scene.objects_mut()[0].transform.translation.x += 20.0;
    refresh_supports(&mut doc, &mut tool, false);

    assert!(
        doc.scene.objects()[0].supports_pending(),
        "the trees grow again"
    );
    let moved = patch_bounds(&doc.scene).mins.x - was.mins.x;
    assert!(
        (moved - 20.0).abs() < 1e-4,
        "expected the patch 20 mm along x, it went {moved}"
    );
}

fn patch_bounds(scene: &Scene) -> core_geometry::Aabb {
    let (painted, _) = scene.objects()[0].supports.patches();
    (painted.and_then(|patch| patch.aabb())).expect("a painted face has a patch")
}

#[test]
fn trees_grown_for_supports_since_changed_are_grown_again() {
    let mut scene = supported_cube();
    let table = [SupportProfile::shipped("medium").expect("the medium preset ships")];
    let mut growth = SupportGrowth::default();

    growth.start(&scene.objects()[0], &table);
    let object = &mut scene.objects_mut()[0];
    object
        .supports
        .add(Vec3::new(3.0, 0.0, 5.0), object.transform, 0);
    settle(&mut growth, &mut scene);

    let object = &scene.objects()[0];
    assert!(object.supports.needs_growing(object.transform, &table));
}

#[test]
fn a_failed_growth_is_reported_and_not_tried_again_until_the_supports_change() {
    let mut scene = supported_cube();
    let table = [SupportProfile::shipped("medium").expect("the medium preset ships")];
    let object = &scene.objects()[0];
    let asked = Asked {
        model: object.id,
        revision: object.supports.revision(),
        transform: object.transform,
        table: table.to_vec(),
    };
    let mut growth = SupportGrowth::default();
    growth.runs.push(Run {
        asked,
        job: Worker::spawn(|_, _| Err(Some("out of memory".to_owned())), || Err(None)),
    });
    let mut status = Status::Idle;
    while growth.poll(&mut scene, &mut status) {
        std::thread::yield_now();
    }
    assert!(status.is_error(), "the failure is told: {status:?}");

    growth.start(&scene.objects()[0], &table);
    assert!(growth.runs.is_empty(), "the same asking is not run again");

    let object = &mut scene.objects_mut()[0];
    object
        .supports
        .add(Vec3::new(3.0, 0.0, 5.0), object.transform, 0);
    growth.poll(&mut scene, &mut status);
    growth.start(&scene.objects()[0], &table);
    assert_eq!(growth.runs.len(), 1, "an edit is grown again");
}

#[test]
fn a_model_failing_at_one_place_after_another_keeps_only_its_last_failure() {
    let mut scene = supported_cube();
    let table = [SupportProfile::shipped("medium").expect("the medium preset ships")];
    let (model, revision) = (
        scene.objects()[0].id,
        scene.objects()[0].supports.revision(),
    );
    let mut growth = SupportGrowth::default();
    for x_mm in [0.0, 20.0, 40.0] {
        growth.runs.push(Run {
            asked: Asked {
                model,
                revision,
                transform: Transform::from_translation(Vec3::new(x_mm, 0.0, 10.0)),
                table: table.to_vec(),
            },
            job: Worker::spawn(|_, _| Err(Some("out of memory".to_owned())), || Err(None)),
        });
        while growth.poll(&mut scene, &mut Status::Idle) {
            std::thread::yield_now();
        }
    }
    assert_eq!(growth.failed.len(), 1, "a drag does not pile failures up");
}
