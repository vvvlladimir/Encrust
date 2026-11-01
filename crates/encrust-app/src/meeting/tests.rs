use super::*;
use crate::scene::fixtures::scene_of_boxes;
use core_geometry::Vec3;

/// Runs frames until the search under way has ended, as the frames after an edit do.
fn settle(meeting: &mut Meeting, scene: &Scene) {
    for _ in 0..10_000 {
        meeting.start(scene, true);
        if !meeting.poll() && meeting.run.is_none() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    panic!("the plate was still being searched after ten seconds");
}

#[test]
fn two_boxes_standing_into_each_other_meet_and_a_third_apart_does_not() {
    let scene = scene_of_boxes(&[
        (Vec3::ZERO, Vec3::splat(10.0)),
        (Vec3::new(5.0, 2.0, 0.0), Vec3::new(15.0, 8.0, 10.0)),
        (Vec3::new(40.0, 0.0, 0.0), Vec3::new(50.0, 10.0, 10.0)),
    ]);
    let mut meeting = Meeting::default();
    settle(&mut meeting, &scene);
    let ids: Vec<ObjectId> = scene.objects().iter().map(|object| object.id).collect();

    assert_eq!(
        meeting.met(&scene),
        &[[ids[0], ids[1]]],
        "they share x = 5..10 mm"
    );
    assert_eq!(meeting.met_by(&scene, ids[1]), vec![ids[0]]);
    assert!(meeting.met_by(&scene, ids[2]).is_empty());
}

#[test]
fn a_model_moved_apart_is_not_named_while_the_plate_is_searched_again() {
    let mut scene = scene_of_boxes(&[
        (Vec3::ZERO, Vec3::splat(10.0)),
        (Vec3::new(5.0, 2.0, 0.0), Vec3::new(15.0, 8.0, 10.0)),
    ]);
    let mut meeting = Meeting::default();
    settle(&mut meeting, &scene);
    scene.objects_mut()[1].transform.translation.x += 20.0;

    assert!(
        meeting.met(&scene).is_empty(),
        "the answer was for the plate before the move"
    );
    settle(&mut meeting, &scene);
    assert!(meeting.met(&scene).is_empty(), "20 mm on, they stand apart");
}
