use super::*;
use core_geometry::{Mesh, Orientation, PlacedHit, Transform, Vec3, diagnose};
use core_volume::Shell;
use std::sync::Arc;

use crate::scene::{ImportSummary, Imported};

fn scene_with_a_tetrahedron() -> Scene {
    let mesh = Mesh::new(
        vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(20.0, 0.0, 0.0),
            Vec3::new(0.0, 20.0, 0.0),
            Vec3::new(0.0, 0.0, 20.0),
        ],
        vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
    );
    let mut scene = Scene::default();
    scene.insert(Imported::new(
        "tetrahedron".to_owned(),
        Arc::new(mesh.clone()),
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
    ));
    scene
}

#[test]
fn a_model_with_its_own_wall_is_hollowed_to_it() {
    let mut scene = scene_with_a_tetrahedron();
    let tool = HollowTool::default();
    let plate_mm = tool.state.thickness_mm;
    let mut own = tool.state.clone();
    own.thickness_mm = plate_mm + 1.0;
    scene.objects_mut()[0].own.hollow = Some(own);

    let tasks = crate::job::hollow_tasks(&scene, |object| tool.settings_for(object));
    assert_eq!(tasks.len(), 1);
    assert!(
        (tasks[0].settings.thickness_mm - (plate_mm + 1.0)).abs() < 1e-6,
        "the model's own wall, not the plate's {plate_mm} mm"
    );
}

#[test]
fn a_click_takes_the_windows_blocker_size_on_a_model_with_its_own_settings() {
    let mut scene = scene_with_a_tetrahedron();
    let tool = HollowTool {
        blocker: BlockerSize {
            fit: false,
            reach_mm: 3.0,
        },
        ..HollowTool::default()
    };
    let mut own = tool.state.clone();
    own.thickness_mm += 1.0;
    scene.objects_mut()[0].own.hollow = Some(own);
    // Under (5, 5) the tetrahedron is 10 mm tall, so a fitted ball would be 5 mm.
    let hit = PlacedHit {
        point: Vec3::new(5.0, 5.0, 0.0),
        normal: Vec3::NEG_Z,
        distance: 0.0,
        face: 0,
    };

    let blocker = tool
        .blocker_for(&scene.objects()[0], &hit, false)
        .expect("the model is not flattened");
    assert!(
        (blocker.radius_mm - 3.0).abs() < 1e-6,
        "the window's fixed 3 mm, whatever the model keeps of its own: {}",
        blocker.radius_mm
    );
}

#[test]
fn turning_a_model_with_an_open_floor_makes_its_shell_stale_and_a_closed_one_not() {
    let mut scene = scene_with_a_tetrahedron();
    let mut tool = HollowTool::default();
    for open_floor in [true, false] {
        tool.state.open_floor = open_floor;
        let object = &mut scene.objects_mut()[0];
        object.transform = Transform::default();
        let built = object.hollow.asking(&tool.settings_for(object));
        object.hollow.take(Shell {
            mesh: Arc::new(Mesh::default()),
            cavity: 0..0,
            cavity_mm3: 0.0,
            voxel_mm: 0.2,
            coarsened: false,
            shape: core_geometry::Mat3::IDENTITY,
            settings: built,
        });
        object.transform.rotation = core_geometry::Quat::from_rotation_x(1.0);
        let object = &scene.objects()[0];
        assert_eq!(
            object
                .hollow
                .is_stale(&tool.settings_for(object), object.transform),
            open_floor,
            "an open floor faces the plate, a closed shell does not care which way it turns"
        );
    }
}

#[test]
fn a_finished_run_asks_once_for_the_drainage_check_that_follows_it() {
    let mut scene = scene_with_a_tetrahedron();
    let mut tool = HollowTool::default();
    let mut status = Status::default();
    tool.start(&scene).expect("a model is on the plate");
    while tool.poll(&mut scene, &mut status) {
        std::thread::yield_now();
    }

    assert!(
        tool.take_hollowed(),
        "a run put a new cavity on the plate, so what an older check found is stale"
    );
    assert!(
        !tool.take_hollowed(),
        "and it looks once per run, not once per frame"
    );
}

#[test]
fn digging_a_channel_into_a_hollow_model_hollows_it_again_and_nothing_else_does() {
    let mut scene = scene_with_a_tetrahedron();
    let mut tool = HollowTool::default();
    assert!(
        !tool.rebuild(&scene),
        "a solid model has no shell to rebuild"
    );

    let object = &mut scene.objects_mut()[0];
    let built = object.hollow.asking(&tool.settings());
    object.hollow.take(Shell {
        mesh: Arc::new(Mesh::default()),
        cavity: 0..0,
        cavity_mm3: 0.0,
        voxel_mm: 0.2,
        coarsened: false,
        shape: core_geometry::Mat3::IDENTITY,
        settings: built,
    });
    assert!(!tool.rebuild(&scene), "the shell is what it was built from");

    let object = &mut scene.objects_mut()[0];
    object
        .hollow
        .add_channel_point(Vec3::new(5.0, 5.0, 0.0), Vec3::NEG_Z, Transform::default());
    object
        .hollow
        .add_channel_point(Vec3::new(10.0, 5.0, 0.0), Vec3::NEG_Z, Transform::default());
    assert!(object.hollow.finish_channel(
        core_volume::HoleSize::round(2.0, 0.0, 1.0),
        Transform::default()
    ));
    assert!(tool.rebuild(&scene), "the channel needs its sleeve now");
    assert!(tool.job.is_some());
}

#[test]
fn a_start_on_walls_hollows_only_the_models_named_at_their_wall() {
    use crate::scene::fixtures::scene_of_boxes;

    let mut scene = scene_of_boxes(&[
        (Vec3::ZERO, Vec3::splat(20.0)),
        (Vec3::new(30.0, 0.0, 0.0), Vec3::new(50.0, 20.0, 20.0)),
    ]);
    let (named, other) = (scene.objects()[0].id, scene.objects()[1].id);
    let mut tool = HollowTool::default();
    tool.state.thickness_mm = 5.0;
    let mut job = tool
        .start_asked(&scene, &[(named, HollowSettings::for_wall(1.5))])
        .expect("a model is named");
    let outcome = loop {
        match job.poll() {
            Some(outcome) => break outcome,
            None => std::thread::yield_now(),
        }
    };
    tool.land(&mut scene, outcome);

    let asked = scene.get(named).and_then(|object| object.hollow.asked());
    let wall_mm = asked.map(|asked| asked.thickness_mm);
    assert_eq!(wall_mm, Some(1.5), "the wall named, not the panel's 5 mm");
    let left = scene.get(other).expect("on the plate");
    assert!(!left.hollow.is_hollow(), "the model not named stays solid");
}
