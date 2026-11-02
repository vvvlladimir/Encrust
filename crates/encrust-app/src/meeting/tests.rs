use super::*;
use crate::pick::Shown;
use crate::scene::fixtures::scene_of_boxes;

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
    meeting.settle(&scene);
    let ids = ids(&scene);

    assert_eq!(meeting.pairs(&scene), vec![[ids[0], ids[1]]]);
    assert_eq!(meeting.met_by(&scene, ids[1]), vec![ids[0]]);
    assert!(meeting.met_by(&scene, ids[2]).is_empty());
    let seam = meeting.highlight(&scene, Shown::default()).edges;
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
fn the_seam_is_traced_only_between_the_section_cuts() {
    let scene = scene_of_boxes(&[
        (Vec3::ZERO, Vec3::splat(10.0)),
        (Vec3::new(5.0, 2.0, 1.0), Vec3::new(15.0, 8.0, 9.0)),
    ]);
    let mut meeting = Meeting::default();
    meeting.settle(&scene);
    let cut = Shown {
        floor_mm: Some(3.0),
        top_mm: Some(5.0),
    };

    let seam = meeting.highlight(&scene, cut).edges;
    // The frame at x = 10 mm runs z = 1..9; between the cuts only its uprights stand, z = 3..5.
    let ends: Vec<Vec3> = seam
        .iter()
        .flat_map(|mark| mark.edges.iter().flatten().copied())
        .collect();
    assert!(!ends.is_empty(), "the uprights pass between the cuts");
    for end in ends {
        assert!(
            (3.0 - 1e-4..=5.0 + 1e-4).contains(&end.z),
            "every end lies between z = 3 and 5 mm, got {end}"
        );
    }
    let above = Shown {
        floor_mm: Some(9.5),
        top_mm: None,
    };
    assert!(
        meeting.highlight(&scene, above).edges.is_empty(),
        "above the seam, nothing is left of it"
    );
}

#[test]
fn each_press_flies_to_the_next_place_the_pair_asked_for_meets() {
    let scene = scene_of_boxes(&[
        (Vec3::new(10.0, 0.0, 0.0), Vec3::new(20.0, 10.0, 10.0)),
        (Vec3::new(5.0, 2.0, 0.0), Vec3::new(12.0, 8.0, 10.0)),
        (Vec3::new(18.0, 2.0, 0.0), Vec3::new(25.0, 8.0, 10.0)),
    ]);
    let mut meeting = Meeting::default();
    meeting.settle(&scene);
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
fn a_press_on_another_pair_starts_at_its_first_place() {
    let scene = scene_of_boxes(&[
        (Vec3::new(10.0, 0.0, 0.0), Vec3::new(20.0, 10.0, 10.0)),
        (Vec3::new(5.0, 2.0, 0.0), Vec3::new(12.0, 8.0, 10.0)),
        (Vec3::new(18.0, 2.0, 0.0), Vec3::new(25.0, 8.0, 10.0)),
    ]);
    let mut meeting = Meeting::default();
    meeting.settle(&scene);
    let [middle, left, _] = ids(&scene)[..] else {
        panic!("three boxes stand on the plate");
    };

    meeting.fly_to(&scene, |pair| pair == [middle, left]);
    meeting.fly_to(&scene, |pair| pair.contains(&middle));
    let spot = meeting.take_flight().expect("the middle box meets two");
    assert!(
        spot.center.x < 15.0,
        "the middle box's first place, the left meeting, got x = {}",
        spot.center.x
    );
}

#[test]
fn a_model_moved_apart_is_not_named_while_the_plate_is_searched_again() {
    let mut scene = scene_of_boxes(&[
        (Vec3::ZERO, Vec3::splat(10.0)),
        (Vec3::new(5.0, 2.0, 0.0), Vec3::new(15.0, 8.0, 10.0)),
    ]);
    let mut meeting = Meeting::default();
    meeting.settle(&scene);
    scene.objects_mut()[1].transform.translation.x += 20.0;

    assert!(
        meeting.pairs(&scene).is_empty()
            && meeting.highlight(&scene, Shown::default()).edges.is_empty(),
        "the answer was for the plate before the move"
    );
    meeting.settle(&scene);
    assert!(
        meeting.pairs(&scene).is_empty(),
        "20 mm on, they stand apart"
    );
}

#[test]
fn a_seam_past_the_budget_is_drawn_as_an_even_spread_within_it() {
    let seam: Vec<[Vec3; 2]> = (0..25_000)
        .map(|piece| {
            let at = Vec3::X * piece as f32;
            [at, at + Vec3::Y]
        })
        .collect();
    let shown: Vec<[Vec3; 2]> = drawn(&seam).collect();
    assert_eq!(shown.len(), 8_334, "every third of 25 000 pieces");
    assert_eq!(shown[1][0].x, 3.0, "spread along the seam, not its head");
    assert_eq!(
        drawn(&seam[..10]).count(),
        10,
        "a seam within it is drawn whole"
    );
}
