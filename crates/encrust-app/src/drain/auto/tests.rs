use super::*;
use core_geometry::{Quat, Transform, Vec3};
use core_volume::{DrainHole, HoleSize};

use crate::scene::fixtures::hollow_cube;
use crate::scene::{ImportSummary, Imported};
use crate::slicing::Slicing;
use crate::state::Tools;
use crate::tool_settings::ToolSettings;
use crate::undo::{Frame, History};

fn posed() -> Transform {
    Transform {
        translation: Vec3::new(40.0, 30.0, 25.0),
        rotation: Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
        ..Transform::default()
    }
}

/// Two holes in the cube's own -Y and +Y faces, as a search would hand them over.
fn proposed_for(scene: &Scene) -> Proposed {
    let object = &scene.objects()[0];
    let size = HoleSize::round(3.0, 3.0, 1.0);
    let hole = |at: Vec3, axis: Vec3| DrainHole::sized(at, axis, size, 0.0);
    Proposed {
        id: object.id,
        mesh: Arc::clone(object.uncut()),
        transform: object.transform,
        holes: vec![
            hole(Vec3::new(10.0, 0.0, 3.0), Vec3::Y),
            hole(Vec3::new(10.0, 20.0, 17.0), Vec3::NEG_Y),
        ],
        trapped: Vec::new(),
    }
}

fn observe(history: &mut History, scene: &Scene, tools: &Tools, slicing: &Slicing) {
    let frame = Frame {
        settled: true,
        typing: false,
    };
    let settings = ToolSettings::of(tools, slicing);
    history.observe(scene, &settings, slicing.chosen(), frame);
}

#[test]
fn the_holes_a_search_lands_are_one_undo_that_puts_the_old_holes_back() {
    let mut scene = hollow_cube(20.0, 2.0, posed());
    let (mut tools, mut slicing) = (Tools::default(), Slicing::default());
    let mut history = History::default();
    let earlier = DrainHole::sized(
        Vec3::new(0.0, 10.0, 10.0),
        Vec3::X,
        HoleSize::round(2.0, 3.0, 1.0),
        0.0,
    );
    scene.objects_mut()[0].hollow.add_hole(earlier);
    observe(&mut history, &scene, &tools, &slicing);
    let before = scene.objects()[0].hollow.drains().to_vec();

    let proposed = proposed_for(&scene);
    let landed = land(&mut scene, vec![proposed]);
    observe(&mut history, &scene, &tools, &slicing);
    assert_eq!(landed.holes, 2);
    assert_eq!(
        scene.objects()[0].hollow.drains().len(),
        3,
        "added to the one there"
    );

    assert!(history.undo(&mut scene, &mut tools, &mut slicing));
    assert_eq!(
        scene.objects()[0].hollow.drains(),
        before.as_slice(),
        "one undo takes every hole the search drilled back"
    );
}

#[test]
fn a_model_moved_while_the_search_ran_gets_nothing() {
    let mut scene = hollow_cube(20.0, 2.0, posed());
    let proposed = proposed_for(&scene);
    scene.objects_mut()[0].transform.translation.x += 5.0;

    let landed = land(&mut scene, vec![proposed]);
    assert_eq!(
        landed,
        Landed {
            holes: 0,
            trapped: 0,
            moved: 1
        }
    );
    assert!(scene.objects()[0].hollow.drains().is_empty());
}

#[test]
fn what_no_hole_drains_is_marked_as_the_drainage_check_marks_it() {
    let mut scene = hollow_cube(20.0, 2.0, posed());
    let mut proposed = proposed_for(&scene);
    proposed.holes.clear();
    proposed.trapped = vec![core_supports::Trapped {
        at: Vec3::splat(10.0),
        volume_mm3: 4096.0,
        bounds: core_geometry::Aabb::new(Vec3::splat(2.0), Vec3::splat(18.0)),
    }];

    let landed = land(&mut scene, vec![proposed]);
    assert_eq!(landed.trapped, 1);
    assert_eq!(scene.objects()[0].traps.found().len(), 1, "painted red");
}

#[test]
fn a_search_cancelled_leaves_the_plate_as_it_was() {
    let mut scene = hollow_cube(20.0, 2.0, posed());
    let mut tool = DrainTool::default();
    let mut status = Status::default();

    tool.start_auto(&scene, 0.5).expect("a model to search");
    assert!(tool.auto.is_some());
    tool.cancel_auto(&mut status);
    assert!(!tool.poll(&mut scene, &mut status), "nothing left running");
    assert!(scene.objects()[0].hollow.drains().is_empty());
    assert!(scene.objects()[0].traps.found().is_empty());
    assert!(
        !tool.checked,
        "a search given up says nothing about drainage"
    );
}

#[test]
fn an_empty_plate_has_nothing_to_search() {
    let mut tool = DrainTool::default();
    assert!(tool.start_auto(&Scene::default(), 0.5).is_err());
    assert!(tool.auto.is_none());
}

#[test]
fn a_search_run_through_drains_the_cube_and_says_the_plate_is_checked() {
    let mut scene = hollow_cube(20.0, 2.0, posed());
    let mut tool = DrainTool::default();
    let mut status = Status::default();

    tool.start_auto(&scene, 0.5).expect("a model to search");
    while tool.poll(&mut scene, &mut status) {
        std::thread::yield_now();
    }
    assert_eq!(
        scene.objects()[0].hollow.drains().len(),
        2,
        "a drain and a vent"
    );
    assert!(tool.checked, "the search's last scan is a drainage check");
    assert_eq!(status, Status::Info("2 holes drilled".to_owned()));
}

#[test]
fn a_search_on_named_models_ignores_what_is_picked() {
    let mut scene = hollow_cube(20.0, 2.0, posed());
    let cube = scene.objects()[0].id;
    let other = core_geometry::cuboid(Vec3::splat(60.0), Vec3::splat(70.0));
    let summary = ImportSummary::made(core_geometry::diagnose(&other));
    let other = Imported::new(
        "box".to_owned(),
        Arc::new(other),
        Transform::default(),
        summary,
    );
    let picked = scene.insert(other);
    scene.select(Some(picked));
    let mut tool = DrainTool::default();

    let mut job = tool
        .search_on(&scene, &[cube], 0.5, true)
        .expect("the cube is named");
    let found = loop {
        match job.poll() {
            Some(DrainsOutcome::Found(found)) => break found,
            Some(other) => panic!("the search ended {other:?}"),
            None => std::thread::yield_now(),
        }
    };
    tool.land_found(&mut scene, found);
    let drilled = |id| scene.get(id).map(|object| object.hollow.drains().len());
    assert_eq!(
        drilled(cube),
        Some(2),
        "a drain and a vent in the cube named"
    );
    assert_eq!(drilled(picked), Some(0), "nothing in the box picked");
}

#[test]
fn a_mirrored_cube_is_drained_through_its_own_walls() {
    let mirrored = Transform {
        scale: Vec3::new(-1.0, 1.0, 1.0),
        ..posed()
    };
    let mut scene = hollow_cube(20.0, 2.0, mirrored);
    let mut tool = DrainTool::default();
    let mut status = Status::default();
    tool.start_auto(&scene, 0.5).expect("a model to search");
    while tool.poll(&mut scene, &mut status) {
        std::thread::yield_now();
    }

    let object = &scene.objects()[0];
    assert_eq!(
        object.hollow.drains().len(),
        2,
        "a drain and a vent, got {status:?}"
    );
    assert!(object.traps.found().is_empty(), "nothing left trapped");
    for hole in object.hollow.drains() {
        let inside = hole.at.cmpge(Vec3::splat(-0.01)) & hole.at.cmple(Vec3::splat(20.01));
        assert!(
            inside.all(),
            "{:?} lies on the cube in its own space",
            hole.at
        );
    }
}
