use core_engine::Cutting;

use super::*;
use crate::preview::tests::{preview_with_stack, scene_with_a_model, settings};

#[test]
fn a_quick_check_waits_for_the_plate_to_stand_still() {
    let mut preview = Preview::default();
    let at = Instant::now();
    assert_eq!(preview.settles_in(at), None, "no plate seen yet");
    preview.note_plate(1, at);
    assert_eq!(preview.settles_in(at), Some(SETTLE));
    preview.note_plate(1, at + SETTLE / 2);
    assert_eq!(
        preview.settles_in(at + SETTLE),
        Some(Duration::ZERO),
        "the same plate seen again keeps its wait"
    );
}

#[test]
fn a_change_drops_the_cut_started_for_the_quick_check_alone() {
    let mut preview = preview_with_stack(2);
    preview.note_plate(1, Instant::now());
    preview
        .build_in_background(&scene_with_a_model(), Cutting::uniform(0.5))
        .expect("the plate has a model on it");
    assert!(preview.is_building() && !preview.is_cutting_in_view());

    preview.note_plate(2, Instant::now());
    assert!(!preview.is_building(), "the plate it was cut for is gone");

    preview
        .build(&scene_with_a_model(), Cutting::uniform(0.5))
        .expect("the plate has a model on it");
    preview.note_plate(3, Instant::now());
    assert!(
        preview.is_cutting_in_view(),
        "a cut the user waits on stays"
    );
}

#[test]
fn a_cut_is_tried_once_for_each_plate() {
    let mut preview = Preview::default();
    assert!(preview.try_cut_for(1));
    assert!(!preview.try_cut_for(1), "a failed cut is not started again");
    assert!(preview.try_cut_for(2));
}

#[test]
fn a_quick_check_reads_every_layer_on_the_coarser_panel() {
    let mut preview = preview_with_stack(4);
    let fold = Fold::default();
    preview.measure_quick(&settings(), fold);
    while preview.poll_quick() {
        std::thread::yield_now();
    }
    let quick = preview
        .quick_measured(&settings(), fold)
        .expect("the quick check lands");
    assert_eq!(quick.layer_count(), 4);
    // The 4 x 4 x 2 mm box on 0.4 mm squares, within the rounding of an edge grey.
    assert!(
        (quick.volume_mm3() - 32.0).abs() < 0.1,
        "4 x 4 x 2 mm is 32 mm³; got {}",
        quick.volume_mm3()
    );
    assert!(
        preview.measured(&settings(), fold).is_none(),
        "the full check is still to come"
    );
}
