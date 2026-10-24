use std::sync::Arc;

use core_geometry::{Transform, Vec3, cuboid, diagnose};

use super::*;
use crate::scene::{ImportSummary, Imported};

fn a_block_with_holes(at: &[Scalar]) -> (Scene, ObjectId, DrainTool) {
    let mesh = cuboid(Vec3::ZERO, Vec3::splat(10.0));
    let mut scene = Scene::default();
    let id = scene.insert(Imported::new(
        "block".to_owned(),
        Arc::new(mesh.clone()),
        Transform::default(),
        ImportSummary::made(diagnose(&mesh)),
    ));
    let tool = DrainTool::default();
    drill(&mut scene, id, &tool, at);
    (scene, id, tool)
}

fn drill(scene: &mut Scene, id: ObjectId, tool: &DrainTool, at: &[Scalar]) {
    let object = scene.get_mut(id).expect("the block is on the plate");
    let (mesh, bvh) = (Arc::clone(&object.mesh), Arc::clone(&object.bvh));
    for x in at {
        object.hollow.add_drain(
            &mesh,
            &bvh,
            Vec3::new(*x, 5.0, 10.0),
            Vec3::Z,
            tool.size(),
            Transform::default(),
        );
    }
}

/// Runs frames until no plug is due or being cut, as the frames after an edit do.
fn lay(tool: &mut DrainTool, scene: &mut Scene, status: &mut Status) {
    for _ in 0..10_000 {
        tool.lay_plugs(scene, 0.0, true, 0);
        if !tool.laying_plugs() {
            return;
        }
        let ended = tool.poll_plugs();
        for ended in &ended {
            DrainTool::land_plugs(scene, ended, Vec2::new(200.0, 120.0), status);
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    panic!("the plugs were still being cut after ten seconds");
}

fn plugs_of(scene: &Scene, id: ObjectId) -> Vec<&SceneObject> {
    scene
        .objects()
        .iter()
        .filter(|object| object.plug_of == Some(id))
        .collect()
}

#[test]
fn a_plug_for_each_hole_stands_on_the_plate_clear_of_its_model() {
    let (mut scene, id, mut tool) = a_block_with_holes(&[3.0, 7.0]);
    lay(&mut tool, &mut scene, &mut Status::default());

    let plugs = plugs_of(&scene, id);
    assert_eq!(plugs.len(), 2, "one plug a hole");
    assert_eq!(scene.selection(), [id], "the block stays picked");
    let edge = scene.get(id).and_then(SceneObject::world_bounds);
    let edge = edge.expect("the block has bounds").maxs.x;
    for plug in plugs {
        let bounds = plug.world_bounds().expect("a plug has bounds");
        assert!(
            bounds.mins.z.abs() < 1e-4,
            "{} stands on the plate",
            plug.name
        );
        assert!(bounds.mins.x > edge, "{} is clear of the block", plug.name);
        assert!(
            (bounds.maxs.z - 3.0).abs() < 1e-3,
            "a solid block's plug is as deep as its 3 mm hole, got {}",
            bounds.maxs.z
        );
    }
}

#[test]
fn plugs_follow_the_holes_and_are_left_alone_while_nothing_moves() {
    let (mut scene, id, mut tool) = a_block_with_holes(&[3.0]);
    let mut status = Status::default();
    lay(&mut tool, &mut scene, &mut status);
    let first = plugs_of(&scene, id)[0].id;
    lay(&mut tool, &mut scene, &mut status);
    assert_eq!(
        plugs_of(&scene, id)[0].id,
        first,
        "nothing moved, nothing is laid"
    );

    drill(&mut scene, id, &tool, &[7.0]);
    lay(&mut tool, &mut scene, &mut status);
    assert_eq!(
        plugs_of(&scene, id).len(),
        2,
        "a second hole, a second plug"
    );

    let object = scene.get_mut(id).expect("the block is on the plate");
    object.hollow.clear_drains();
    lay(&mut tool, &mut scene, &mut status);
    assert!(plugs_of(&scene, id).is_empty(), "no hole, no plug");
    assert_eq!(scene.objects().len(), 1, "only the block is left");
}

#[test]
fn plugs_cut_for_holes_taken_back_before_they_land_are_let_go() {
    let (mut scene, id, mut tool) = a_block_with_holes(&[3.0]);
    let mut status = Status::default();
    tool.lay_plugs(&mut scene, 0.0, true, 0);
    assert!(tool.laying_plugs(), "the plug is being cut");

    // An undo puts the block back as it was before the hole.
    let object = scene.get_mut(id).expect("the block is on the plate");
    object.hollow.clear_drains();
    lay(&mut tool, &mut scene, &mut status);
    assert!(!tool.laying_plugs(), "the cut is let go");
    assert!(plugs_of(&scene, id).is_empty(), "and lays nothing");
}

#[test]
fn plugs_cut_from_a_mesh_replaced_before_they_land_are_cut_again() {
    let (mut scene, id, mut tool) = a_block_with_holes(&[3.0]);
    tool.lay_plugs(&mut scene, 0.0, true, 0);
    let object = scene.get_mut(id).expect("the block is on the plate");
    object.mesh = Arc::new((*object.mesh).clone());

    lay(&mut tool, &mut scene, &mut Status::default());
    assert_eq!(plugs_of(&scene, id).len(), 1, "the plug is laid");
    let read = tool.read.as_ref().expect("the model is kept read");
    let mesh = &scene.get(id).expect("the block").mesh;
    assert_eq!(
        read.mesh.as_ptr(),
        Arc::as_ptr(mesh),
        "cut from the mesh the block has now"
    );
}

/// Whether two boxes across the plate overlap, touching aside.
fn overlap(one: Aabb, other: Aabb) -> bool {
    one.mins.x < other.maxs.x
        && other.mins.x < one.maxs.x
        && one.mins.y < other.maxs.y
        && other.mins.y < one.maxs.y
}

#[test]
fn a_plug_keeps_clear_of_a_model_standing_where_it_would_go() {
    let (mut scene, id, mut tool) = a_block_with_holes(&[3.0, 7.0]);
    // A second block just right of the first, where the plugs are laid in a row.
    let mesh = cuboid(Vec3::ZERO, Vec3::splat(10.0));
    let neighbour = scene.insert(Imported::new(
        "neighbour".to_owned(),
        Arc::new(mesh.clone()),
        Transform::from_translation(Vec3::new(12.0, 0.0, 0.0)),
        ImportSummary::made(diagnose(&mesh)),
    ));
    lay(&mut tool, &mut scene, &mut Status::default());

    let next = scene.get(neighbour).expect("the neighbour");
    let beside = next.world_bounds().expect("the neighbour has bounds");
    let plugs: Vec<Aabb> = plugs_of(&scene, id)
        .iter()
        .map(|plug| plug.world_bounds().expect("a plug has bounds"))
        .collect();
    assert_eq!(plugs.len(), 2, "one plug a hole");
    for plug in &plugs {
        assert!(
            !overlap(*plug, beside),
            "no plug stands on the neighbour: {plug:?}"
        );
        assert!(
            plug.mins.x >= 2.0 - 1e-4 && plug.mins.y >= 2.0 - 1e-4,
            "on the plate"
        );
    }
    assert!(!overlap(plugs[0], plugs[1]), "nor on one another");
}

#[test]
fn a_model_at_the_edge_of_the_plate_gets_its_plug_on_the_plate() {
    let (mut scene, id, mut tool) = a_block_with_holes(&[3.0]);
    // The test plate is 200 mm across: the block's right side 1 mm short of its edge.
    scene.get_mut(id).expect("the block").transform =
        Transform::from_translation(Vec3::new(189.0, 50.0, 0.0));
    lay(&mut tool, &mut scene, &mut Status::default());

    let plug = plugs_of(&scene, id)[0]
        .world_bounds()
        .expect("a plug has bounds");
    assert!(
        plug.maxs.x <= 200.0 - 2.0 + 1e-4,
        "the plug stays inside the plate's margin, got {plug:?}"
    );
}

#[test]
fn a_plug_with_no_room_on_the_plate_is_said_to_have_none() {
    let (mut scene, id, mut tool) = a_block_with_holes(&[3.0]);
    let mut status = Status::default();
    tool.lay_plugs(&mut scene, 0.0, true, 0);
    while tool.laying_plugs() {
        for ended in tool.poll_plugs() {
            // A plate no wider than the block leaves no room beside it.
            DrainTool::land_plugs(&mut scene, &ended, Vec2::new(10.0, 10.0), &mut status);
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(plugs_of(&scene, id).len(), 1, "the plug is still laid");
    assert!(
        matches!(&status, Status::Noted(_, rest) if rest.is_empty()),
        "and the user told it found no room: {status:?}"
    );
}

#[test]
fn a_plug_is_numbered_after_its_hole_when_an_earlier_one_has_none() {
    let (mut scene, id, _) = a_block_with_holes(&[3.0, 7.0]);
    let holes = scene.get(id).expect("the block").hollow.cut().0.to_vec();
    let made = [Mesh::default(), cuboid(Vec3::ZERO, Vec3::ONE)];
    replace(&mut scene, id, &holes, &made, Vec2::new(200.0, 120.0));

    let plugs = plugs_of(&scene, id);
    assert_eq!(plugs.len(), 1, "the empty plug is not laid");
    let name = tr!("drain-plug-name", name = "block".to_owned(), index = 2);
    assert_eq!(plugs[0].name, name, "the one laid is the second hole's");
}

#[test]
fn plugs_started_again_after_a_stale_cut_land_in_the_edit_that_asked_for_them() {
    let (mut scene, _, mut tool) = a_block_with_holes(&[5.0]);
    tool.lay_plugs(&mut scene, 0.0, true, 0);
    tool.state.plug_gap_mm = 0.5;
    tool.lay_plugs(&mut scene, 0.0, true, 1);

    let mut landed = Vec::new();
    for frame in 2..10_000 {
        tool.lay_plugs(&mut scene, 0.0, true, frame);
        if !tool.laying_plugs() {
            break;
        }
        for ended in tool.poll_plugs() {
            landed.push(ended.asked_by());
            DrainTool::land_plugs(
                &mut scene,
                &ended,
                Vec2::new(200.0, 120.0),
                &mut Status::default(),
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(
        landed,
        [1],
        "the gap's edit asked for the plugs that landed"
    );
}

#[test]
fn no_plug_is_cut_in_the_middle_of_a_drag() {
    let (mut scene, id, mut tool) = a_block_with_holes(&[3.0]);
    tool.lay_plugs(&mut scene, 0.0, false, 0);
    assert!(
        !tool.laying_plugs(),
        "nothing starts while the pointer is down"
    );

    lay(&mut tool, &mut scene, &mut Status::default());
    assert_eq!(
        plugs_of(&scene, id).len(),
        1,
        "the plug follows once it is up"
    );
}

#[test]
fn a_gap_changed_while_plugs_are_cut_waits_for_them_and_reads_the_model_once() {
    let (mut scene, id, mut tool) = a_block_with_holes(&[5.0]);
    tool.lay_plugs(&mut scene, 0.0, true, 0);
    tool.state.plug_gap_mm = 0.5;
    tool.lay_plugs(&mut scene, 0.0, true, 0);
    assert_eq!(tool.laying.len(), 1, "one cut a model at a time");

    lay(&mut tool, &mut scene, &mut Status::default());
    let plugs = plugs_of(&scene, id);
    assert_eq!(
        plugs.len(),
        1,
        "the plug cut for the old gap is thrown away"
    );
    let width = plugs[0]
        .world_bounds()
        .expect("a plug has bounds")
        .extents()
        .x;
    // A 3 mm hole, 0.5 mm in on each side: under 2 mm across, where 0.15 left 2.7.
    assert!(
        width < 2.05,
        "the plug is cut for the last gap, got {width} mm"
    );
    let read = tool.read.as_ref().expect("the model is kept read");
    let mesh = &scene.get(id).expect("the block").mesh;
    assert_eq!(
        read.mesh.as_ptr(),
        Arc::as_ptr(mesh),
        "and kept for the block as it is"
    );
}

#[test]
fn a_model_opened_with_its_plugs_keeps_them() {
    let (mut scene, id, mut tool) = a_block_with_holes(&[3.0]);
    let mut status = Status::default();
    lay(&mut tool, &mut scene, &mut status);
    let first = plugs_of(&scene, id)[0].id;
    // A project read back carries the link, not what the plugs were laid for.
    scene.get_mut(id).expect("the block").plugs_laid = None;

    lay(&mut tool, &mut scene, &mut status);
    assert_eq!(
        plugs_of(&scene, id)[0].id,
        first,
        "the plug read back is kept"
    );
}

#[test]
fn a_plug_deleted_before_saving_stays_deleted_once_opened() {
    let (mut scene, id, mut tool) = a_block_with_holes(&[3.0]);
    let mut status = Status::default();
    lay(&mut tool, &mut scene, &mut status);
    let plug = plugs_of(&scene, id)[0].id;
    scene.remove(plug);
    scene.get_mut(id).expect("the block").plugs_laid = None;

    lay(&mut tool, &mut scene, &mut status);
    assert!(
        plugs_of(&scene, id).is_empty(),
        "the project is opened as it was saved"
    );
}

#[test]
fn a_copy_of_a_model_gets_plugs_of_its_own() {
    let (mut scene, id, mut tool) = a_block_with_holes(&[3.0]);
    let mut status = Status::default();
    lay(&mut tool, &mut scene, &mut status);
    let copy = scene
        .duplicate(id, Vec3::new(0.0, 30.0, 0.0))
        .expect("the block is there to copy");

    lay(&mut tool, &mut scene, &mut status);
    assert_eq!(
        plugs_of(&scene, copy).len(),
        1,
        "the copy's hole is plugged"
    );
    assert_eq!(plugs_of(&scene, id).len(), 1, "the original keeps its own");
}

#[test]
fn a_copied_plate_keeps_its_plugs_apart_from_the_original() {
    let (mut scene, id, mut tool) = a_block_with_holes(&[3.0]);
    let mut status = Status::default();
    lay(&mut tool, &mut scene, &mut status);
    let copied = scene.duplicate_plate(0).expect("plate 0 is there to copy");
    let model = scene
        .on_plate(copied)
        .find(|object| object.plug_of.is_none())
        .map(|object| object.id)
        .expect("the block was copied");
    assert_eq!(
        plugs_of(&scene, model).len(),
        1,
        "the copied plug is the copy's"
    );

    drill(&mut scene, id, &tool, &[7.0]);
    lay(&mut tool, &mut scene, &mut status);
    assert_eq!(
        plugs_of(&scene, id).len(),
        2,
        "the original lays its own again"
    );
    assert_eq!(
        plugs_of(&scene, model).len(),
        1,
        "and leaves the copy's alone"
    );
}
