use super::*;

const SLACK: Noise = Noise {
    goals: [0.0; OrientGoal::ALL.len()],
    seen: 0.0,
};
/// A twentieth of a millilitre of support, half a newton of pull, 0.3 mm of stack and half
/// a contact on show, as a proof reads.
const PROVEN: Noise = Noise {
    goals: [0.05, 0.5, 0.3],
    seen: 0.5,
};
const GOAL: OrientPreset = OrientPreset::Goal(OrientGoal::LeastPeel);
const SHORTEST: OrientPreset = OrientPreset::Goal(OrientGoal::ShortestPrint);
const PEEL: OrientPreset = OrientPreset::Goal(OrientGoal::LeastPeel);
const FEWEST: OrientPreset = OrientPreset::Goal(OrientGoal::FewestSupports);
const AUTO: OrientPreset = OrientPreset::Auto;
const SUPPORT: usize = OrientGoal::FewestSupports.index();
const PULL: usize = OrientGoal::LeastPeel.index();
const STACK: usize = OrientGoal::ShortestPrint.index();

/// `preset` held to a clean surface.
fn clean(preset: OrientPreset) -> Asked {
    Asked {
        preset,
        clean_surface: true,
    }
}

fn reading(fatal: f64, unheld: Option<f64>, primary: f64, tiebreak: f64) -> Readings {
    Readings {
        fatal,
        unheld,
        goals: [primary; OrientGoal::ALL.len()],
        seen: 0.0,
        suction: 0.0,
        tearing: 0.0,
        roomless: 0.0,
        tiebreak: [tiebreak, 0.0],
        standing: false,
    }
}

/// Every single-goal preset.
fn every_goal() -> impl Iterator<Item = OrientPreset> {
    OrientGoal::ALL.into_iter().map(OrientPreset::Goal)
}

#[test]
fn what_fails_the_print_loses_whatever_it_reads() {
    let readings = [reading(1.0, None, 1.0, 0.0), reading(0.0, None, 100.0, 1.0)];
    for preset in every_goal() {
        assert_eq!(rank(&readings, preset, &SLACK)[0], 1, "{preset:?}");
    }
}

fn sucking(primary: f64) -> Readings {
    Readings {
        suction: 1.0,
        ..reading(0.0, None, primary, 0.0)
    }
}

fn tearing(primary: f64) -> Readings {
    Readings {
        tearing: 1.0,
        ..reading(0.0, None, primary, 0.0)
    }
}

#[test]
fn a_single_goal_takes_its_least_reading_whatever_it_cups_or_tears() {
    let sound = reading(0.0, None, 90.0, 1.0);
    for preset in every_goal() {
        assert_eq!(
            rank(&[sucking(60.0), sound], preset, &SLACK)[0],
            0,
            "{preset:?}"
        );
        assert_eq!(
            rank(&[tearing(60.0), sound], preset, &SLACK)[0],
            0,
            "{preset:?}"
        );
    }
}

#[test]
fn a_single_goal_takes_its_least_reading_whatever_is_left_hanging() {
    let loose = reading(0.0, Some(12.0), 10.0, 1.0);
    let roomless = Readings {
        roomless: 40.0,
        ..reading(0.0, Some(0.0), 10.0, 1.0)
    };
    let held = reading(0.0, Some(0.0), 11.0, 0.0);
    for preset in every_goal() {
        assert_eq!(rank(&[loose, held], preset, &SLACK)[0], 0, "{preset:?}");
        assert_eq!(rank(&[roomless, held], preset, &SLACK)[0], 0, "{preset:?}");
    }
}

#[test]
fn a_single_goal_has_no_band_over_its_least() {
    let readings = [
        reading(0.0, None, 105.0, 0.0),
        reading(0.0, None, 100.0, 1.0),
        reading(0.0, None, 101.0, 2.0),
    ];
    for preset in every_goal() {
        assert_eq!(rank(&readings, preset, &PROVEN), [1, 2, 0], "{preset:?}");
    }
}

#[test]
fn a_single_goal_turns_for_any_less_of_its_reading() {
    let mut now = reading(0.0, None, 100.0, 1.0);
    now.standing = true;
    let slightly = reading(0.0, None, 99.0, 0.0);
    let same = reading(0.0, None, 100.0, 0.0);
    for preset in every_goal() {
        assert_eq!(
            rank(&[slightly, now], preset, &PROVEN)[0],
            0,
            "{preset:?}: 1 % less is less"
        );
        assert_eq!(
            rank(&[same, now], preset, &PROVEN)[0],
            1,
            "{preset:?}: as little stays"
        );
    }
}

#[test]
fn a_turn_that_fails_less_is_always_worth_it() {
    let mut now = reading(1.0, None, 10.0, 1.0);
    now.standing = true;
    let sound = reading(0.0, None, 10.0, 0.0);
    assert_eq!(rank(&[now, sound], GOAL, &SLACK)[0], 1);
}

#[test]
fn of_two_ways_of_standing_still_the_better_is_what_a_turn_is_weighed_against() {
    let mut supported = reading(0.0, None, 46.0, 0.0);
    supported.standing = true;
    let turned = reading(0.0, None, 0.0, 1.0);
    let mut bare = reading(0.0, None, 0.0, 2.0);
    bare.standing = true;
    assert_eq!(
        rank(&[supported, turned, bare], GOAL, &SLACK)[0],
        2,
        "set bare as it stands, nothing is left for a turn to win"
    );
}

#[test]
fn nothing_to_rank_ranks_nothing() {
    assert!(rank(&[], GOAL, &SLACK).is_empty());
    assert!(rulings(&[], GOAL, &SLACK).is_empty());
}

#[test]
fn each_place_is_ruled_by_the_first_thing_it_lost_on() {
    let readings = [
        reading(1.0, Some(0.0), 10.0, 0.0),
        reading(0.0, Some(0.0), 50.0, 2.0),
        reading(0.0, Some(0.0), 10.0, 4.0),
        reading(0.0, Some(0.0), 10.0, 3.0),
    ];
    assert_eq!(
        rulings(&readings, GOAL, &SLACK),
        [
            Ruling::Fatal,
            Ruling::OutOfBand,
            Ruling::TieBreak,
            Ruling::Chosen
        ]
    );
}

#[test]
fn standing_still_is_ruled_stayed_and_the_turn_it_beat_not_worth_it() {
    let mut now = reading(0.0, None, 100.0, 1.0);
    now.standing = true;
    let alike = reading(0.0, None, 100.0, 0.0);
    assert_eq!(
        rulings(&[alike, now], GOAL, &SLACK),
        [Ruling::NotWorthTurning, Ruling::Stayed]
    );
}

/// A candidate reading `primary` on its goal and `supports` on the fewest supports.
fn spending(goal: usize, primary: f64, supports: f64) -> Readings {
    let mut r = reading(0.0, Some(0.0), 0.0, 0.0);
    r.goals[goal] = primary;
    r.goals[SUPPORT] = supports;
    r
}

#[test]
fn the_least_peel_takes_the_least_pull_however_little_and_whatever_its_support() {
    let readings = [spending(PULL, 1.5, 9.0), spending(PULL, 1.0, 30.0)];
    assert_eq!(rank(&readings, PEEL, &PROVEN)[0], 1);
}

#[test]
fn the_shortest_print_takes_the_shortest_stack_whatever_its_support() {
    let readings = [spending(STACK, 100.0, 20.0), spending(STACK, 100.5, 1.0)];
    assert_eq!(rank(&readings, SHORTEST, &PROVEN)[0], 0);
}

/// A sound candidate reading `support` ml, `pull` newtons and `stack` millimetres.
fn weighing(support: f64, pull: f64, stack: f64) -> Readings {
    let mut r = reading(0.0, Some(0.0), 0.0, 0.0);
    r.goals[SUPPORT] = support;
    r.goals[PULL] = pull;
    r.goals[STACK] = stack;
    r
}

#[test]
fn auto_takes_a_pose_good_on_every_goal_over_a_cheaper_trade() {
    // 1.0 ml is within a tenth and 0.05 ml of 0.91 ml, and 100 mm within a fiftieth and
    // a millimetre of 97.1 mm; 104 mm is not, though its product of shares is lower.
    let readings = [
        weighing(0.91, 5.0, 104.0),
        weighing(1.0, 5.0, 100.0),
        weighing(1.3, 5.0, 97.1),
    ];
    assert_eq!(rank(&readings, AUTO, &PROVEN)[0], 1);
}

#[test]
fn auto_weighs_shares_not_units() {
    // The two finalists of a 98 mm disc with pockets: eight times the support on its face,
    // five times the stack on its edge. The larger share loses whatever the units are.
    let edge = weighing(0.79, 0.9, 103.0);
    let face = weighing(6.19, 8.4, 21.3);
    assert_eq!(rank(&[face, edge], AUTO, &PROVEN)[0], 1);

    let in_microlitres = |mut r: Readings| {
        r.goals[SUPPORT] *= 1000.0;
        r
    };
    let mut noise = PROVEN;
    noise.goals[SUPPORT] *= 1000.0;
    assert_eq!(
        rank(&[in_microlitres(face), in_microlitres(edge)], AUTO, &noise)[0],
        1,
        "a goal read in another unit chooses the same"
    );
}

#[test]
fn auto_refuses_a_cup_and_a_tearing_pull_whatever_they_save() {
    let sound = weighing(4.0, 20.0, 100.0);
    let cupped = Readings {
        suction: 1.0,
        ..weighing(1.0, 5.0, 50.0)
    };
    let tearing = Readings {
        tearing: 1.0,
        ..weighing(1.0, 5.0, 50.0)
    };
    assert_eq!(rank(&[cupped, tearing, sound], AUTO, &PROVEN)[0], 2);
}

#[test]
fn auto_offers_every_trade_no_other_pose_beats_and_rules_the_rest_dominated() {
    let readings = [
        weighing(1.0, 5.0, 100.0),
        weighing(4.0, 5.0, 40.0),
        weighing(4.5, 5.0, 45.0),
        weighing(2.0, 5.0, 60.0),
    ];
    let rulings = rulings(&readings, AUTO, &PROVEN);
    assert_eq!(
        rulings[2],
        Ruling::Dominated,
        "40 mm and 4.0 ml beat it on both"
    );
    assert_eq!(
        rulings.iter().filter(|ruling| ruling.is_offered()).count(),
        3,
        "the other three each trade support for time"
    );
    assert_eq!(*rank(&readings, AUTO, &PROVEN).last().expect("four"), 2);
}

#[test]
fn auto_ignores_a_goal_every_pose_reads_alike() {
    // Every pull is under enough: what is left is support against time.
    let readings = [weighing(1.0, 9.0, 50.0), weighing(1.0, 1.0, 60.0)];
    assert_eq!(rank(&readings, AUTO, &PROVEN)[0], 0);
}

#[test]
fn auto_stays_where_the_model_stands_good_on_every_goal() {
    let mut now = weighing(1.02, 5.0, 101.0);
    now.standing = true;
    let turned = weighing(1.0, 5.0, 100.0);
    assert_eq!(
        rulings(&[turned, now], AUTO, &PROVEN),
        [Ruling::NotWorthTurning, Ruling::Stayed]
    );
    let clearly = weighing(0.3, 5.0, 80.0);
    assert_eq!(rank(&[clearly, now], AUTO, &PROVEN)[0], 0);
}

/// A sound candidate with `seen` contacts on show and `support` ml of support.
fn showing(seen: f64, support: f64) -> Readings {
    Readings {
        seen,
        ..weighing(support, 5.0, 50.0)
    }
}

#[test]
fn unasked_what_shows_weighs_nothing() {
    let readings = [showing(9.0, 1.0), showing(0.0, 1.0)];
    for preset in OrientPreset::all() {
        assert_eq!(rank(&readings, preset, &PROVEN), [0, 1], "{preset:?}");
    }
}

#[test]
fn a_clean_surface_holds_the_preset_to_the_fewest_on_show_and_a_contact_over() {
    let readings = [showing(9.0, 0.5), showing(3.0, 2.0), showing(4.2, 1.5)];
    assert_eq!(
        rank(&readings, clean(FEWEST), &PROVEN)[0],
        2,
        "4.2 on show is within 3 * 1.1 + 1, and takes less resin than 3"
    );
    assert_eq!(rank(&readings, clean(AUTO), &PROVEN)[0], 2);
}

#[test]
fn a_pose_showing_more_is_offered_only_for_far_more_of_what_the_preset_wins() {
    let readings = [showing(0.0, 4.0), showing(6.0, 1.0), showing(6.0, 3.8)];
    assert_eq!(
        rulings(&readings, clean(FEWEST), &PROVEN),
        [Ruling::Chosen, Ruling::Traded, Ruling::Shows],
        "a quarter of the 4 ml is worth six contacts on show; 0.2 ml is not"
    );
    assert_eq!(
        rulings(&readings, clean(AUTO), &PROVEN),
        [Ruling::Chosen, Ruling::Traded, Ruling::Shows]
    );
}

#[test]
fn a_trade_another_trade_beats_on_both_is_not_offered() {
    let readings = [showing(0.0, 4.0), showing(6.0, 1.0), showing(8.0, 1.2)];
    assert_eq!(
        rulings(&readings, clean(FEWEST), &PROVEN),
        [Ruling::Chosen, Ruling::Traded, Ruling::Shows]
    );
}

#[test]
fn a_turn_off_what_shows_is_always_worth_it() {
    let mut now = showing(6.0, 1.0);
    now.standing = true;
    let hidden = showing(0.0, 1.5);
    for preset in OrientPreset::all() {
        assert_eq!(
            rank(&[now, hidden], clean(preset), &PROVEN)[0],
            1,
            "{preset:?}"
        );
    }
}
