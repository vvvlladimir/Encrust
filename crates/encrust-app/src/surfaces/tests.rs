use core_geometry::{Transform, Vec3, cuboid, diagnose};

use super::*;
use crate::scene::{ImportSummary, Imported};

fn hollow_block() -> (Scene, ObjectId) {
    let mesh = cuboid(Vec3::ZERO, Vec3::splat(10.0));
    let mut scene = Scene::default();
    let id = scene.insert(Imported::new(
        "block".to_owned(),
        Arc::new(mesh.clone()),
        Transform::default(),
        ImportSummary::made(diagnose(&mesh)),
    ));
    let object = scene.get_mut(id).expect("the block is on the plate");
    let settings = object
        .hollow
        .asking(&core_volume::HollowSettings::for_wall(1.0));
    let mut shell = mesh.clone();
    shell.append(&cuboid(Vec3::splat(1.0), Vec3::splat(9.0)));
    object.hollow.take(core_volume::Shell {
        mesh: Arc::new(shell),
        cavity: 12..24,
        cavity_mm3: 512.0,
        voxel_mm: 0.05,
        coarsened: false,
        shape: object.transform.unturned(),
        settings,
    });
    (scene, id)
}

fn settle(surfaces: &mut SupportSurfaces, scene: &mut Scene) {
    surfaces.start(scene);
    while surfaces.poll(scene, &mut Status::Idle) {
        std::thread::yield_now();
    }
}

#[test]
fn a_shell_stands_supports_against_it_once_its_hierarchy_is_built_on_a_worker() {
    let (mut scene, id) = hollow_block();
    let shell = Arc::clone(
        scene
            .get(id)
            .and_then(|object| object.hollow.shell())
            .expect("the block is hollow"),
    );
    let mut surfaces = SupportSurfaces::default();
    surfaces.start(&mut scene);
    let object = scene.get(id).expect("the block is on the plate");
    assert!(
        !Arc::ptr_eq(object.support_surface().0, &shell),
        "the model itself stands in while the shell's hierarchy is built"
    );
    settle(&mut surfaces, &mut scene);
    let object = scene.get(id).expect("the block is on the plate");
    assert!(
        Arc::ptr_eq(object.support_surface().0, &shell),
        "the shell once its hierarchy is built"
    );
    assert!(object.supported_inside(), "and its cavity is known");
}

#[test]
fn a_hole_takes_the_tips_it_drills_out_as_it_is_drilled_and_a_kept_surface_none() {
    let (mut scene, id) = hollow_block();
    let object = scene.get_mut(id).expect("the block is on the plate");
    let transform = object.transform;
    object.supports.add(Vec3::new(5.0, 5.0, 0.0), transform, 0);
    object.supports.add(Vec3::new(2.0, 2.0, 0.0), transform, 0);
    // 3 mm wide and 0.5 mm deep, shy of the 1 mm wall: its floor is printed.
    let size = core_volume::HoleSize::round(3.0, 0.5, 1.0);
    let hole = core_volume::DrainHole {
        kind: core_volume::HoleKind::AtDepth,
        ..core_volume::DrainHole::sized(Vec3::new(5.0, 5.0, 0.0), Vec3::Z, size, 0.05)
    };
    assert!(object.drill_hole(hole), "the underside takes a hole");
    let contacts = |scene: &Scene| -> Vec<Vec3> {
        let object = scene.get(id).expect("the block is on the plate");
        object
            .supports
            .points()
            .iter()
            .map(|point| point.contact)
            .collect()
    };
    assert_eq!(
        contacts(&scene),
        [Vec3::new(2.0, 2.0, 0.0)],
        "the tip in the 3 mm hole goes with the click, the one 4 mm off it stays"
    );

    // A tip set on the hole's floor afterwards stands on what prints, and an undo of the
    // drilling is not undone again once its surface is kept.
    let floor = Vec3::new(5.0, 5.0, 0.5);
    let object = scene.get_mut(id).expect("the block is on the plate");
    object.supports.add(floor, transform, 0);
    settle(&mut SupportSurfaces::default(), &mut scene);
    assert_eq!(
        contacts(&scene),
        [Vec3::new(2.0, 2.0, 0.0), floor],
        "keeping the drilled surface takes no support away"
    );
}

#[test]
fn a_model_solid_again_lets_its_shell_go_on_the_next_frame() {
    let (mut scene, id) = hollow_block();
    let mut surfaces = SupportSurfaces::default();
    settle(&mut surfaces, &mut scene);
    let object = scene.get_mut(id).expect("the block is on the plate");
    object.hollow.clear();
    surfaces.start(&mut scene);
    let object = scene.get(id).expect("the block is on the plate");
    assert!(
        !object.supported_inside(),
        "a solid model has no cavity to hold up"
    );
    assert!(
        !surfaces.poll(&mut scene, &mut Status::Idle),
        "and nothing is left to build"
    );
}

#[test]
fn a_surface_that_failed_to_build_is_reported_and_not_built_again() {
    let (mut scene, id) = hollow_block();
    let shell = Arc::clone(
        scene
            .get(id)
            .and_then(|object| object.hollow.shell())
            .expect("the block is hollow"),
    );
    let mut surfaces = SupportSurfaces::default();
    surfaces.runs.push(Run {
        model: id,
        mesh: Arc::downgrade(&shell),
        job: Worker::spawn(|_, _| Err(Some("out of memory".to_owned())), || Err(None)),
    });
    let mut status = Status::Idle;
    while surfaces.poll(&mut scene, &mut status) {
        std::thread::yield_now();
    }
    assert!(status.is_error(), "the failure is told: {status:?}");

    surfaces.start(&mut scene);
    assert!(
        surfaces.runs.is_empty(),
        "the same surface is not built again"
    );
}
