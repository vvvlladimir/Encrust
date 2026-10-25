use super::*;

const SLACK: Noise = [0.0; OrientGoal::ALL.len()];
/// A twentieth of a millilitre of support, half a newton of pull and 0.3 mm of stack, as
/// a proof reads.
const PROVEN: Noise = [0.05, 0.5, 0.3];
/// A goal ranked with its bands, read past enough pull to have nothing to fall back on.
const GOAL: OrientPreset = OrientPreset::Goal(OrientGoal::LeastPeel);
const SHORTEST: OrientPreset = OrientPreset::Goal(OrientGoal::ShortestPrint);
const PEEL: OrientPreset = OrientPreset::Goal(OrientGoal::LeastPeel);
const FEWEST: OrientPreset = OrientPreset::Goal(OrientGoal::FewestSupports);
const AUTO: OrientPreset = OrientPreset::Auto;
const SUPPORT: usize = OrientGoal::FewestSupports.index();
const PULL: usize = OrientGoal::LeastPeel.index();
const STACK: usize = OrientGoal::ShortestPrint.index();

fn reading(fatal: f64, unheld: Option<f64>, primary: f64, tiebreak: f64) -> Readings {
    Readings {
        fatal,
        unheld,
        goals: [primary; OrientGoal::ALL.len()],
        suction: 0.0,
        tearing: 0.0,
        roomless: 0.0,
        tiebreak: [tiebreak, 0.0],
        standing: false,
    }
}

#[test]
fn what_fails_the_print_loses_whatever_it_reads() {
    let readings = [reading(1.0, None, 1.0, 0.0), reading(0.0, None, 100.0, 1.0)];
    assert_eq!(rank(&readings, GOAL, &SLACK)[0], 1);
    assert_eq!(rank(&readings, FEWEST, &SLACK)[0], 1);
}

#[test]
fn what_is_left_hanging_past_the_band_loses_to_what_is_held() {
    let readings = [
        reading(0.0, Some(12.0), 10.0, 0.0),
        reading(0.0, Some(3.0), 50.0, 1.0),
    ];
    assert_eq!(
        rank(&readings, GOAL, &SLACK)[0],
        1,
        "3 * 1.25 + 2 is under 12"
    );
}

#[test]
fn within_the_band_the_tiebreak_decides() {
    let readings = [
        reading(0.0, None, 100.0, 5.0),
        reading(0.0, None, 105.0, 1.0),
        reading(0.0, None, 120.0, 0.0),
    ];
    assert_eq!(rank(&readings, GOAL, &SLACK), [1, 0, 2]);
}

#[test]
fn within_the_band_the_fewest_left_hanging_come_first() {
    let readings = [
        reading(0.0, Some(3.0), 100.0, 0.0),
        reading(0.0, Some(1.0), 105.0, 1.0),
    ];
    assert_eq!(rank(&readings, GOAL, &SLACK)[0], 1);
}

fn sucking(primary: f64) -> Readings {
    Readings {
        suction: 1.0,
        ..reading(0.0, None, primary, 0.0)
    }
}

#[test]
fn the_fewest_supports_take_the_least_support_whatever_it_cups() {
    let sound = reading(0.0, None, 90.0, 1.0);
    assert_eq!(rank(&[sucking(60.0), sound], FEWEST, &SLACK)[0], 0);
}

#[test]
fn contacts_with_no_room_hang_for_every_goal_but_the_fewest_supports() {
    let roomless = Readings {
        roomless: 40.0,
        ..reading(0.0, Some(0.0), 60.0, 0.0)
    };
    let roomy = reading(0.0, Some(0.0), 90.0, 1.0);
    assert_eq!(rank(&[roomless, roomy], FEWEST, &SLACK)[0], 0);
    assert_eq!(rank(&[roomless, roomy], PEEL, &SLACK)[0], 1);
}

#[test]
fn other_goals_refuse_a_cup_whatever_it_saves() {
    let sound = reading(0.0, None, 90.0, 1.0);
    assert_eq!(rank(&[sucking(60.0), sound], PEEL, &SLACK)[0], 1);
    assert_eq!(
        rulings(&[sucking(60.0), sound], PEEL, &SLACK),
        [Ruling::Fatal, Ruling::Chosen]
    );
}

#[test]
fn the_fewest_supports_take_the_least_support_whatever_is_left_hanging() {
    let loose = reading(0.0, Some(12.0), 10.0, 1.0);
    let held = reading(0.0, Some(0.0), 11.0, 0.0);
    assert_eq!(rank(&[loose, held], FEWEST, &SLACK)[0], 0);
}

#[test]
fn the_fewest_supports_have_no_band_over_the_least() {
    let readings = [
        reading(0.0, None, 105.0, 0.0),
        reading(0.0, None, 100.0, 1.0),
        reading(0.0, None, 101.0, 2.0),
    ];
    assert_eq!(rank(&readings, FEWEST, &PROVEN), [1, 2, 0]);
}

#[test]
fn the_fewest_supports_turn_for_any_less_support() {
    let mut now = reading(0.0, None, 100.0, 1.0);
    now.standing = true;
    let slightly = reading(0.0, None, 99.0, 0.0);
    assert_eq!(
        rank(&[slightly, now], FEWEST, &PROVEN)[0],
        0,
        "1 % less is less"
    );
    let same = reading(0.0, None, 100.0, 0.0);
    assert_eq!(rank(&[same, now], FEWEST, &PROVEN)[0], 1, "as little stays");
}

#[test]
fn a_turn_has_to_win_more_than_the_noise_of_its_unit() {
    let mut now = reading(0.0, None, 12.0, 1.0);
    now.standing = true;
    let turned = reading(0.0, None, 11.0, 0.0);
    let coarse = [0.05, 2.0, 0.3];
    assert_eq!(
        rank(&[turned, now], GOAL, &coarse)[0],
        1,
        "a twelfth of the reading is past the share, but not past two newtons of noise"
    );
}

#[test]
fn standing_still_wins_over_a_turn_that_gains_too_little() {
    let mut now = reading(0.0, None, 100.0, 1.0);
    now.standing = true;
    let slightly = reading(0.0, None, 98.0, 0.0);
    assert_eq!(
        rank(&[slightly, now], GOAL, &SLACK)[0],
        1,
        "2 % is not worth a turn"
    );
    let clearly = reading(0.0, None, 80.0, 0.0);
    assert_eq!(rank(&[clearly, now], GOAL, &SLACK)[0], 0, "20 % is");
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
        reading(0.0, Some(20.0), 10.0, 1.0),
        reading(0.0, Some(0.0), 50.0, 2.0),
        reading(0.0, Some(0.0), 10.0, 4.0),
        reading(0.0, Some(0.0), 10.0, 3.0),
    ];
    assert_eq!(
        rulings(&readings, GOAL, &SLACK),
        [
            Ruling::Fatal,
            Ruling::Unheld,
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
    let slightly = reading(0.0, None, 98.0, 0.0);
    assert_eq!(
        rulings(&[slightly, now], GOAL, &SLACK),
        [Ruling::NotWorthTurning, Ruling::Stayed]
    );
}

/// A candidate pulling `pull` newtons that the fewest supports reads as `supports`,
/// tie-broken as a proof does, on the pull to its noise.
fn pulling(pull: f64, supports: f64) -> Readings {
    let mut r = reading(0.0, Some(0.0), pull, (pull / 0.5).round());
    r.goals[SUPPORT] = supports;
    r
}

#[test]
fn under_enough_pull_the_least_peel_takes_the_least_support() {
    let readings = [pulling(1.0, 9.0), pulling(6.0, 3.0), pulling(14.0, 1.0)];
    assert_eq!(
        rank(&readings, PEEL, &PROVEN),
        [1, 0, 2],
        "14 N is past 10 N and its band"
    );
}

#[test]
fn past_enough_the_least_peel_takes_the_least_pull_out_of_its_band() {
    let readings = [pulling(25.0, 1.0), pulling(15.0, 9.0)];
    assert_eq!(rank(&readings, PEEL, &PROVEN)[0], 1);
}

#[test]
fn a_model_pulling_little_enough_turns_only_to_spare_support() {
    let mut now = pulling(4.0, 10.0);
    now.standing = true;
    let lighter = pulling(1.0, 9.8);
    assert_eq!(
        rank(&[lighter, now], PEEL, &PROVEN)[0],
        1,
        "3 N less of a pull already under enough wins nothing"
    );
    let spare = pulling(1.0, 5.0);
    assert_eq!(
        rank(&[spare, now], PEEL, &PROVEN)[0],
        0,
        "half the support is worth it"
    );
}

#[test]
fn a_turn_to_spare_support_has_to_spare_more_than_its_noise() {
    let mut now = pulling(2.9, 0.58);
    now.standing = true;
    let turned = pulling(2.9, 0.54);
    assert_eq!(
        rank(&[turned, now], PEEL, &PROVEN)[0],
        1,
        "0.04 ml is under the 0.05 ml a support reading is good to"
    );
}

#[test]
fn past_enough_the_least_peel_takes_less_pull_over_less_support() {
    let readings = [pulling(28.4, 3.4), pulling(26.2, 5.1)];
    assert_eq!(
        rank(&readings, PEEL, &PROVEN)[0],
        1,
        "both in the band over 26.2 N, neither with enough: the pull decides"
    );
}

/// A candidate standing `stack_mm` tall that the fewest supports reads as `supports`,
/// tie-broken as a proof does, on the stack to its noise.
fn stacking(stack_mm: f64, supports: f64) -> Readings {
    let noise = PROVEN[STACK];
    let mut r = reading(0.0, Some(0.0), stack_mm, (stack_mm / noise).round());
    r.goals[SUPPORT] = supports;
    r
}

#[test]
fn the_shortest_print_takes_the_shortest_stack_whatever_its_support() {
    let readings = [stacking(100.0, 20.0), stacking(101.0, 1.0)];
    assert_eq!(rank(&readings, SHORTEST, &PROVEN)[0], 0);
}

#[test]
fn the_shortest_print_counts_a_pull_past_the_limit_for_nothing_and_a_cup_as_fatal() {
    let tearing = Readings {
        tearing: 1.0,
        ..stacking(80.0, 5.0)
    };
    let sound = stacking(100.0, 5.0);
    assert_eq!(rank(&[sound, tearing], SHORTEST, &PROVEN)[0], 1);
    assert_eq!(rank(&[sound, tearing], GOAL, &PROVEN)[0], 0);
    let cupped = Readings {
        suction: 1.0,
        ..stacking(80.0, 5.0)
    };
    assert_eq!(rank(&[sound, cupped], SHORTEST, &PROVEN)[0], 0);
}

#[test]
fn as_fast_within_a_fiftieth_and_a_millimetre_the_shortest_print_stays() {
    let mut now = stacking(102.0, 5.0);
    now.standing = true;
    let faster = stacking(100.0, 5.0);
    assert_eq!(rank(&[faster, now], SHORTEST, &PROVEN)[0], 1);
    let past = stacking(95.0, 5.0);
    assert_eq!(
        rank(&[past, now], SHORTEST, &PROVEN)[0],
        0,
        "102 mm is past 95 * 1.02 + 1"
    );
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
    noise[SUPPORT] *= 1000.0;
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
