use super::*;
use crate::scene::{ImportSummary, Imported};
use crate::status::Change;
use core_geometry::{Mesh, Orientation, Quat, Transform, Vec3, diagnose};
use std::sync::Arc;

/// Axis-aligned box, twelve triangles, spanning 0..size on every axis.
fn cuboid(size: Vec3) -> Mesh {
    core_geometry::cuboid(Vec3::ZERO, size)
}

fn scene_with(mesh: Mesh) -> Scene {
    let mut scene = Scene::default();
    scene.insert(Imported::new(
        "slab".to_owned(),
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

/// Runs a job to its end, the way the window's frame loop would.
fn settle(tool: &mut OrientTool, scene: &mut Scene, status: &mut Status) {
    while tool.poll(scene, &BuildPlate::default(), status) {
        std::thread::yield_now();
    }
}

/// Presses Auto-orient over the whole plate and waits for what it does.
fn press(tool: &mut OrientTool, scene: &mut Scene, status: &mut Status) {
    let plate = BuildPlate::default();
    tool.start(
        scene,
        &plate,
        None,
        |_| SupportProfile::shipped("medium").expect("the medium preset ships"),
        None,
        status,
    )
    .expect("a visible model can be oriented");
    settle(tool, scene, status);
}

#[test]
fn a_model_oriented_again_after_a_change_is_not_turned_the_second_time() {
    let mut scene = scene_with(cuboid(Vec3::new(40.0, 40.0, 4.0)));
    let mut tool = OrientTool::default();
    let mut status = Status::default();
    press(&mut tool, &mut scene, &mut status);
    tool.settings.preset = core_plate::OrientGoal::FewestSupports.into();
    press(&mut tool, &mut scene, &mut status);
    assert!(
        matches!(&status, Status::Info(text) if text.starts_with("Everything already")),
        "the second run finds it standing as the first left it, got {status:?}"
    );
}

#[test]
fn pressed_again_with_nothing_changed_it_steps_rather_than_searches() {
    let mut scene = scene_with(cuboid(Vec3::new(40.0, 20.0, 4.0)));
    let mut tool = OrientTool::default();
    let mut status = Status::default();
    press(&mut tool, &mut scene, &mut status);
    let plate = BuildPlate::default();

    tool.start(
        &mut scene,
        &plate,
        None,
        |_| SupportProfile::shipped("medium").expect("the medium preset ships"),
        None,
        &mut status,
    )
    .expect("a visible slab can be oriented");
    assert!(!tool.is_running(), "nothing new to ask, so no run");

    scene.objects_mut()[0].lift_to(3.0);
    tool.start(
        &mut scene,
        &plate,
        None,
        |_| SupportProfile::shipped("medium").expect("the medium preset ships"),
        None,
        &mut status,
    )
    .expect("a visible slab can be oriented");
    assert!(tool.is_running(), "a new lift is a new question");
    settle(&mut tool, &mut scene, &mut status);
}

#[test]
fn a_turned_model_lands_back_on_the_plate() {
    let mut scene = scene_with(cuboid(Vec3::new(40.0, 40.0, 4.0)));
    let mut tool = OrientTool::default();
    let mut status = Status::default();

    press(&mut tool, &mut scene, &mut status);

    let bounds = scene.world_bounds().expect("the slab is still there");
    assert!(
        bounds.mins.z.abs() < 1e-3,
        "the model stands on the plate, not through it, got {}",
        bounds.mins.z
    );
    assert!(!status.is_error(), "the run finished");
}

#[test]
fn a_turned_model_stays_over_the_spot_it_stood_on() {
    // A 6 x 6 x 80 column lying along X, its own origin at one end: the shortest arc
    // that stands it up swings it about that end, off the spot and past the plate.
    let mut scene = scene_with(cuboid(Vec3::new(6.0, 6.0, 80.0)));
    let id = scene.objects()[0].id;
    scene.objects_mut()[0].transform = Transform {
        rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
        translation: Vec3::new(20.0, 40.0, 6.0),
        ..Transform::default()
    };
    let middle = |scene: &Scene| {
        let bounds = scene.world_bounds().expect("the column is there");
        ((bounds.mins + bounds.maxs) / 2.0).truncate()
    };
    let before = middle(&scene);
    let upright = Pose {
        rotation: Quat::IDENTITY,
        on_plate: true,
        costs: None,
    };
    let column = scene.get_mut(id).expect("the column is there");
    stand(column, upright, 0.0, &BuildPlate::default());

    let after = middle(&scene);
    assert!(
        after.abs_diff_eq(before, 1e-3),
        "{before} before, {after} after"
    );
    let bounds = scene.world_bounds().expect("the column is there");
    assert!(
        bounds.mins.z.abs() < 1e-3,
        "on the plate, got {}",
        bounds.mins.z
    );
}

#[test]
fn a_model_turned_near_the_edge_is_slid_back_onto_the_plate() {
    // Standing 4 mm from the plate's left edge: laid on its side over the same middle,
    // the 80 mm column would reach 36 mm past that edge.
    let mut scene = scene_with(cuboid(Vec3::new(6.0, 6.0, 80.0)));
    let id = scene.objects()[0].id;
    scene.objects_mut()[0].transform.translation = Vec3::new(1.0, 30.0, 0.0);
    let on_side = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    let lying = Pose {
        rotation: on_side,
        on_plate: true,
        costs: None,
    };
    let column = scene.get_mut(id).expect("the column is there");
    stand(column, lying, 0.0, &BuildPlate::default());

    let bounds = scene.world_bounds().expect("the column is there");
    let plate = BuildPlate::default();
    assert!(
        bounds.mins.x >= -1e-3 && bounds.maxs.x <= plate.x_mm + 1e-3,
        "inside the plate's width, got {:?}",
        bounds
    );
}

#[test]
fn a_resized_model_is_judged_at_the_size_it_prints() {
    // A 100 mm cube squashed to 4 mm is a slab pulling some 98 N lying flat, which
    // the least peel turns off its face.
    let mut scene = scene_with(cuboid(Vec3::splat(100.0)));
    scene.objects_mut()[0].transform.scale = Vec3::new(1.0, 1.0, 0.04);
    let mut tool = OrientTool::default();
    tool.settings.preset = core_plate::OrientGoal::LeastPeel.into();
    let mut status = Status::default();

    press(&mut tool, &mut scene, &mut status);

    let bounds = scene.world_bounds().expect("the slab is still there");
    let height_mm = bounds.maxs.z - bounds.mins.z;
    assert!(
        height_mm > 10.0,
        "the 4 mm slab does not lie flat, got {height_mm}"
    );
}

#[test]
fn stepping_through_the_offered_poses_turns_the_model_and_wraps_round() {
    let mut scene = scene_with(cuboid(Vec3::new(40.0, 20.0, 4.0)));
    let id = scene.objects()[0].id;
    let pose = |rotation, supports_ml| Pose {
        rotation,
        on_plate: true,
        costs: Some(Costs {
            supports_ml,
            pull_n: 5.0,
            stack_mm: 10.0,
        }),
    };
    let on_edge = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    let mut tool = OrientTool {
        offers: vec![(
            Turn {
                id,
                poses: vec![pose(Quat::IDENTITY, 1.0), pose(on_edge, 2.0)],
                lift_mm: 0.0,
            },
            0,
        )],
        ..OrientTool::default()
    };
    let plate = BuildPlate::default();

    let said = tool.step(&mut scene, &plate);
    let turned = scene.objects()[0].transform.rotation;
    assert!(
        turned.abs_diff_eq(on_edge, 1e-5),
        "the second pose, got {turned:?}"
    );
    let Status::Readings(headline, readings) = said else {
        panic!("a proven pose says what it costs, got {said:?}");
    };
    assert_eq!(headline, "Pose 2 of 2");
    assert_eq!(
        readings[0].change,
        Change::Worse,
        "2 ml of supports after 1"
    );
    assert_eq!(readings[1].change, Change::Same, "the same pull");

    tool.step(&mut scene, &plate);
    let turned = scene.objects()[0].transform.rotation;
    assert!(
        turned.abs_diff_eq(Quat::IDENTITY, 1e-5),
        "past the last, the first, got {turned:?}"
    );
}

#[test]
fn a_lift_the_user_asked_for_stands_a_supported_pose_on_it() {
    let mut scene = scene_with(cuboid(Vec3::splat(10.0)));
    scene.objects_mut()[0].lift_to(2.5);
    let tasks = orient_tasks(&scene, None, |_| {
        SupportProfile::shipped("medium").expect("the medium preset ships")
    });
    assert_eq!(tasks[0].support.z_lift_mm, 2.5);

    scene.objects_mut()[0].lift_to(0.0);
    let tasks = orient_tasks(&scene, None, |_| {
        SupportProfile::shipped("medium").expect("the medium preset ships")
    });
    assert_eq!(
        tasks[0].support.z_lift_mm,
        SupportProfile::shipped("medium")
            .expect("the medium preset ships")
            .z_lift_mm,
        "back on the plate, the profile's lift"
    );
}

#[test]
fn an_empty_plate_has_nothing_to_orient() {
    let mut tool = OrientTool::default();
    let scene = Scene::default();
    assert!(tool.blocker(&scene).is_some());
    let mut scene = scene;
    let plate = BuildPlate::default();
    let none =
        |_: &SceneObject| SupportProfile::shipped("medium").expect("the medium preset ships");
    let mut status = Status::default();
    assert!(
        tool.start(&mut scene, &plate, None, none, None, &mut status)
            .is_err()
    );
}

#[test]
fn a_hidden_model_is_not_turned() {
    let mut scene = scene_with(cuboid(Vec3::splat(10.0)));
    scene.objects_mut()[0].visible = false;
    let tool = OrientTool::default();
    assert!(tool.blocker(&scene).is_some());
}

#[test]
fn pointing_at_one_triangle_of_a_cube_side_finds_the_whole_side() {
    let scene = scene_with(cuboid(Vec3::splat(10.0)));
    let object = &scene.objects()[0];
    let mut tool = OrientTool::default();
    // Face 0 is one of the two triangles of the bottom, facing -Z.
    tool.point_at(object, 0);

    let facet = tool.facet.expect("a cube side is flat");
    assert_eq!(facet.faces.len(), 2, "a side of the cube is two triangles");
    assert!(
        facet.normal.abs_diff_eq(-Vec3::Z, 1e-5),
        "got {:?}",
        facet.normal
    );
}
