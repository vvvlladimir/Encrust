use core_geometry::{Bvh, Transform, Vec3, cuboid, diagnose, signed_volume};
use core_volume::HoleSize;

use super::*;
use crate::scene::{ImportSummary, Imported};

/// A 10 mm block with a 3 mm hole 5 mm down into the middle of its top.
fn a_drilled_block() -> (Scene, ObjectId) {
    a_block_drilled_by(HoleSize::round(3.0, 5.0, 1.0))
}

fn a_block_drilled_by(size: HoleSize) -> (Scene, ObjectId) {
    let mesh = cuboid(Vec3::ZERO, Vec3::splat(10.0));
    let mut scene = Scene::default();
    let id = scene.insert(Imported::new(
        "block".to_owned(),
        Arc::new(mesh.clone()),
        Transform::default(),
        ImportSummary::made(diagnose(&mesh)),
    ));
    let object = scene.get_mut(id).expect("the block is on the plate");
    let bvh = Bvh::build(&mesh);
    let at = Vec3::new(5.0, 5.0, 10.0);
    object
        .hollow
        .add_drain(&mesh, &bvh, at, Vec3::Z, size, Transform::default());
    (scene, id)
}

/// Runs frames until no cut is being taken out, as the frames after an edit do.
fn settle(drilling: &mut Drilling, scene: &mut Scene, status: &mut Status) {
    for _ in 0..10_000 {
        drilling.start(scene, true);
        if !drilling.poll(scene, status) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    panic!("the cuts were still being taken out after ten seconds");
}

#[test]
fn a_hole_is_taken_out_of_the_block_it_was_drilled_in() {
    let (mut scene, id) = a_drilled_block();
    let mut drilling = Drilling::default();
    settle(&mut drilling, &mut scene, &mut Status::default());

    let object = scene.get(id).expect("the block");
    let volume = signed_volume(object.printed());
    assert!(
        volume < 1000.0 - 30.0,
        "the 1000 mm3 block less a 3 mm by 5 mm bore of some 35 mm3, got {volume}"
    );
    assert!(!object.hollow.wants_drilling(object.uncut()), "and kept");
}

#[test]
fn cuts_given_up_are_left_to_the_winding_rule_and_not_taken_out_again() {
    let (mut scene, id) = a_drilled_block();
    let mut drilling = Drilling::default();
    drilling.start(&scene, true);
    assert!(drilling.drills(id), "the hole is being taken out");

    drilling.give_up(&mut scene);
    assert!(!drilling.is_running(), "the run is let go");
    let object = scene.get(id).expect("the block is on the plate");
    assert!(
        object.hollow.drilled(object.uncut()).is_none(),
        "nothing exact is kept"
    );
    drilling.start(&scene, true);
    assert!(!drilling.drills(id), "and the cuts are not taken out again");
}

#[test]
fn nothing_is_taken_out_in_the_middle_of_a_drag() {
    let (mut scene, _) = a_drilled_block();
    let mut drilling = Drilling::default();
    drilling.start(&scene, false);
    assert!(
        !drilling.is_running(),
        "nothing starts while the pointer is down"
    );
    settle(&mut drilling, &mut scene, &mut Status::default());
}

#[test]
fn a_cut_for_holes_since_changed_is_not_kept() {
    let (mut scene, id) = a_drilled_block();
    let mut drilling = Drilling::default();
    drilling.start(&scene, true);
    scene.get_mut(id).expect("the block").hollow.clear_drains();
    while drilling.poll(&mut scene, &mut Status::default()) {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let object = scene.get(id).expect("the block");
    assert!(
        Arc::ptr_eq(object.printed(), &object.mesh),
        "a block with no hole left prints as it came"
    );
}

#[test]
fn a_refusal_for_holes_since_changed_is_not_reported() {
    // A hole wider and deeper than the block leaves nothing of it, which the engine refuses.
    let (mut scene, id) = a_block_drilled_by(HoleSize::round(30.0, 20.0, 1.0));
    let mut drilling = Drilling::default();
    drilling.start(&scene, true);
    let object = scene.get_mut(id).expect("the block");
    // An undo step still holds the old cuts.
    let _held = object.hollow.cut_bodies().cloned();
    object.hollow.clear_drains();
    let mut status = Status::default();
    while drilling.poll(&mut scene, &mut status) {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(status, Status::Idle, "the holes it refused are gone");
}

#[test]
fn a_refusal_for_the_holes_standing_is_reported() {
    let (mut scene, _) = a_block_drilled_by(HoleSize::round(30.0, 20.0, 1.0));
    let mut status = Status::default();
    settle(&mut Drilling::default(), &mut scene, &mut status);
    assert!(
        matches!(status, Status::Info(_)),
        "the user is told: {status:?}"
    );
}

#[test]
fn an_undo_step_keeps_no_drilled_mesh() {
    let (mut scene, id) = a_drilled_block();
    let mut drilling = Drilling::default();
    settle(&mut drilling, &mut scene, &mut Status::default());

    let kept = scene.snapshot();
    let object = kept.get(id).expect("the block");
    assert!(
        object.hollow.wants_drilling(object.uncut()),
        "the step drills again"
    );
    let mut put_back = kept.clone();
    put_back.adopt_drilled(&scene);
    let object = put_back.get(id).expect("the block");
    assert!(
        !object.hollow.wants_drilling(object.uncut()),
        "unless the plate it replaces still has it for the same cuts"
    );
}

#[test]
fn a_run_reading_a_mesh_of_its_own_moves_the_bar() {
    let read = ModelRead::new(Arc::new(cuboid(Vec3::ZERO, Vec3::ONE)));
    let own = Counted {
        share: 0.3,
        step: Step::Reading,
    };
    let (step, share) = least_along(std::iter::once((Some(own), &read))).expect("one run");
    assert_eq!(step, Step::Reading);
    assert!((share - 0.3).abs() < 1e-6, "expected 0.3, got {share}");
}
