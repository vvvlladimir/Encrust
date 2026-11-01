use super::*;
use crate::scene::fixtures::scene_of_boxes;

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

fn ids(scene: &Scene) -> Vec<ObjectId> {
    scene.objects().iter().map(|object| object.id).collect()
}

#[test]
fn two_boxes_standing_into_each_other_meet_and_their_seam_is_traced_in_red() {
    let scene = scene_of_boxes(&[
        (Vec3::ZERO, Vec3::splat(10.0)),
        (Vec3::new(5.0, 2.0, 1.0), Vec3::new(15.0, 8.0, 9.0)),
        (Vec3::new(40.0, 0.0, 0.0), Vec3::new(50.0, 10.0, 10.0)),
    ]);
    let mut meeting = Meeting::default();
    settle(&mut meeting, &scene);
    let ids = ids(&scene);

    assert_eq!(meeting.pairs(&scene), vec![[ids[0], ids[1]]]);
    assert_eq!(meeting.met_by(&scene, ids[1]), vec![ids[0]]);
    assert!(meeting.met_by(&scene, ids[2]).is_empty());
    let seam = meeting.highlight(&scene).edges;
    assert_eq!(seam.len(), 1, "the seam, one mark in red");
    assert_eq!(seam[0].colour, theme::scene().meets);
    // The seam is the frame x = 10 mm, y = 2..8, z = 1..9 where the first box's wall cuts
    // through the second.
    for end in seam[0].edges.iter().flatten() {
        assert!(
            (end.x - 10.0).abs() < 1e-4
                && (2.0..=8.0).contains(&end.y)
                && (1.0..=9.0).contains(&end.z),
            "every end lies on the frame, got {end}"
        );
    }
}

#[test]
fn each_press_flies_to_the_next_place_the_pair_asked_for_meets() {
    let scene = scene_of_boxes(&[
        (Vec3::new(10.0, 0.0, 0.0), Vec3::new(20.0, 10.0, 10.0)),
        (Vec3::new(5.0, 2.0, 0.0), Vec3::new(12.0, 8.0, 10.0)),
        (Vec3::new(18.0, 2.0, 0.0), Vec3::new(25.0, 8.0, 10.0)),
    ]);
    let mut meeting = Meeting::default();
    settle(&mut meeting, &scene);
    let middle = ids(&scene)[0];

    let mut flown = Vec::new();
    for _ in 0..3 {
        meeting.fly_to(&scene, |pair| pair.contains(&middle));
        let spot = meeting.take_flight().expect("the middle box meets two");
        flown.push(spot.center.x);
    }
    assert!(
        flown[0] < 15.0 && flown[1] > 15.0,
        "left meeting, then the right: {flown:?}"
    );
    assert_eq!(flown[2], flown[0], "and round to the first");
    assert!(meeting.take_flight().is_none(), "a flight is taken once");
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
        meeting.pairs(&scene).is_empty() && meeting.highlight(&scene).edges.is_empty(),
        "the answer was for the plate before the move"
    );
    settle(&mut meeting, &scene);
    assert!(
        meeting.pairs(&scene).is_empty(),
        "20 mm on, they stand apart"
    );
}
