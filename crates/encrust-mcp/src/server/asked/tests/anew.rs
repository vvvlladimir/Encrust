//! Supports stood again from nothing after a new pose (ADR 0460).

use super::*;

fn turned(earlier: Recipe) -> Recipe {
    Recipe {
        pose: Some(Pose::Rotate {
            degrees: [35.0, -30.0, -10.0],
        }),
        ..earlier
    }
}

fn after_turn(earlier: &Recipe, stood: Stood) -> Option<Vec<Edit>> {
    let recipe = turned(earlier.clone());
    asked_after_check(&recipe, earlier, MODEL, (None, None, None, stood))
}

fn anew_of(asked: &[Edit]) -> Option<(Option<&str>, bool)> {
    match asked {
        [
            Edit::Rotate { .. },
            Edit::Supports {
                profile,
                cavity,
                anew: true,
                only_at: false,
                ..
            },
        ] => Some((profile.as_deref(), *cavity)),
        _ => None,
    }
}

#[test]
fn a_turn_of_a_prepared_model_stands_the_autopilots_supports_anew() {
    let prepared = Recipe {
        prepare: true,
        ..bare()
    };
    // A plate whose trees grow again on opening reports no supports: the recipe still asks them.
    let asked = after_turn(&prepared, (false, true)).expect("a turn and supports are edits");
    assert_eq!(
        anew_of(&asked),
        Some((Some(core_edit::AUTO), true)),
        "auto supports holding up the cavity, stood anew: {asked:?}"
    );
}

#[test]
fn a_turn_of_a_model_on_no_supports_stands_none() {
    let asked = after_turn(&bare(), (false, false)).expect("a turn is an edit");
    assert!(
        matches!(asked.as_slice(), [Edit::Rotate { .. }]),
        "{asked:?}"
    );
}

#[test]
fn a_turn_of_a_model_on_supports_from_the_window_stands_them_anew_in_its_group() {
    let asked = after_turn(&bare(), (true, false)).expect("a turn and supports are edits");
    assert_eq!(anew_of(&asked), Some((None, false)), "{asked:?}");
}

#[test]
fn a_turn_stands_the_supports_the_recipe_asks_anew_with_its_profile() {
    let supported = Recipe {
        supports: Some(Supports {
            profile: "medium".into(),
            cavity: true,
            ..Supports::default()
        }),
        ..bare()
    };
    let asked = after_turn(&supported, (false, false)).expect("a turn and supports are edits");
    assert_eq!(anew_of(&asked), Some((Some("medium"), true)), "{asked:?}");
}

#[test]
fn a_turn_of_a_model_prepared_by_a_recipe_is_left_to_the_flags() {
    let by_recipe = Recipe {
        steps_by: Some("miniatures".into()),
        ..bare()
    };
    assert_eq!(after_turn(&by_recipe, (true, false)), None);
}

#[test]
fn a_new_pose_takes_away_the_support_points_of_the_old_one() {
    let mut asked = Supports {
        profile: "medium".into(),
        at_mm: vec![[1.0, 2.0, 3.0]],
        shown_at_mm: vec![[4.0, 5.0, 6.0]],
        only_at: true,
        ..Supports::default()
    };
    assert!(asked.drop_plate_points(), "there were points");
    let supported = Recipe {
        supports: Some(asked),
        ..bare()
    };
    let asked = after_turn(&supported, (false, false)).expect("a turn and supports are edits");
    assert_eq!(anew_of(&asked), Some((Some("medium"), false)), "{asked:?}");
}
