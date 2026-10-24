use super::*;
use core_engine::Printout;
use core_geometry::Quat;

/// A goal judged with its bands; the fewest supports has none.
const GOAL: OrientGoal = OrientGoal::LeastPeel;
const FEWEST: OrientGoal = OrientGoal::FewestSupports;

/// A printout standing `contacts` contacts on a tenth of a millilitre of support each,
/// pulling a newton each: past the least peel's enough from ten.
fn printout(contacts: usize) -> Printout {
    Printout {
        contacts,
        supports_ml: contacts as f32 / 10.0,
        hanging: 0,
        top_contacts: 0,
        layers: 100,
        stack_mm: 5.0,
        pull_n: contacts as f64,
        growth_mm2: 0.0,
        islands: 0,
        cups: 0,
        peels: 0,
        resin_ml: 1.0,
        overflows: false,
    }
}

fn pose(name: &str, source: Source, contacts: usize, tilt_deg: f32) -> Pose {
    Pose {
        name: name.to_owned(),
        source,
        rotation: Quat::IDENTITY,
        on_plate: false,
        printout: printout(contacts),
        tilt_deg,
        fits: true,
        sits: true,
        seconds: 0.1,
        trace: crate::poses::Trace::default(),
    }
}

fn chosen(contacts: usize, tilt_deg: f32) -> Pose {
    pose("chosen", Source::Goal(GOAL.into()), contacts, tilt_deg)
}

fn broken(poses: &[Pose]) -> Vec<&'static str> {
    broken_for(GOAL, poses)
}

/// What `goal` breaks having chosen the pose called "chosen".
fn broken_for(goal: OrientGoal, poses: &[Pose]) -> Vec<&'static str> {
    let mut poses = poses.to_vec();
    for pose in poses.iter_mut().filter(|pose| pose.name == "chosen") {
        pose.source = Source::Goal(goal.into());
    }
    judge(goal.into(), &poses, Expect::default(), 1000)
        .expect("the goal's pose is among them")
        .broken()
}

#[test]
fn the_best_pose_breaks_nothing() {
    let poses = [pose("as is", Source::Hand, 100, 0.0), chosen(40, 30.0)];
    assert!(broken(&poses).is_empty());
}

#[test]
fn a_cup_the_soundest_pose_avoids_is_fatal_but_to_the_fewest_supports() {
    let mut cupped = chosen(10, 30.0);
    cupped.printout.cups = 1;
    let poses = [pose("as is", Source::Hand, 100, 0.0), cupped];
    assert!(
        broken_for(FEWEST, &poses).is_empty(),
        "a drain hole fixes a cup"
    );
    assert!(broken(&poses).contains(&"fatal"));
}

#[test]
fn islands_past_the_band_of_the_soundest_pose_are_not_held_but_by_the_fewest_supports() {
    let mut loose = chosen(10, 30.0);
    loose.printout.islands = 12;
    let poses = [pose("as is", Source::Hand, 100, 0.0), loose];
    assert_eq!(broken(&poses), ["held", "goal"]);
    assert!(broken_for(FEWEST, &poses).is_empty());
}

#[test]
fn a_pose_on_the_plate_with_something_hanging_is_not_seated_but_to_the_fewest_supports() {
    let mut bare = chosen(0, 30.0);
    bare.on_plate = true;
    bare.printout.hanging = 1;
    let poses = [pose("as is", Source::Hand, 100, 0.0), bare];
    assert!(
        !broken_for(FEWEST, &poses).contains(&"seated"),
        "a contact with no room is placement's to leave"
    );
    assert!(broken(&poses).contains(&"seated"));
}

#[test]
fn a_pose_balanced_on_a_point_is_not_seated() {
    let mut balanced = chosen(0, 30.0);
    balanced.on_plate = true;
    balanced.sits = false;
    let poses = [pose("as is", Source::Hand, 100, 0.0), balanced];
    assert!(broken_for(FEWEST, &poses).contains(&"seated"));
}

#[test]
fn a_pose_out_of_the_volume_fails_when_another_fits() {
    let mut big = chosen(10, 30.0);
    big.fits = false;
    let poses = [pose("as is", Source::Hand, 100, 0.0), big];
    assert!(broken_for(FEWEST, &poses).contains(&"fits"));
}

#[test]
fn a_reading_past_the_band_of_the_best_sound_pose_loses() {
    let poses = [
        pose("oracle 3", Source::Oracle, 50, 40.0),
        pose("as is", Source::Hand, 200, 0.0),
        chosen(80, 30.0),
    ];
    assert_eq!(broken(&poses), ["goal"]);
}

#[test]
fn the_fewest_supports_have_no_band_over_the_best() {
    let poses = [
        pose("oracle 3", Source::Oracle, 50, 40.0),
        pose("as is", Source::Hand, 200, 0.0),
        chosen(52, 30.0),
    ];
    assert_eq!(broken_for(FEWEST, &poses), ["goal"], "5.2 ml against 5.0");
    assert!(broken(&poses).is_empty(), "52 N is within a tenth of 50");
}

#[test]
fn a_turn_that_barely_beats_standing_as_drawn_is_needless_but_to_the_fewest_supports() {
    let poses = [pose("as is", Source::Hand, 100, 0.0), chosen(97, 30.0)];
    assert_eq!(broken(&poses), ["stays"]);
    assert!(
        broken_for(FEWEST, &poses).is_empty(),
        "less support is less"
    );
}

#[test]
fn a_tilt_past_what_a_person_expects_breaks_the_expectation() {
    let poses = [pose("as is", Source::Hand, 100, 0.0), chosen(40, 45.0)];
    let expect = Expect {
        max_tilt_deg: Some(10.0),
        on_plate: Some(false),
    };
    let verdict = judge(GOAL.into(), &poses, expect, 1000).expect("the goal's pose is there");
    assert_eq!(verdict.broken(), ["expected"]);
}

#[test]
fn a_goal_no_measured_pose_could_pass_is_unreachable_not_failed() {
    let poses = [
        pose("as is", Source::Hand, 100, 0.0),
        pose("over", Source::Hand, 10, 170.0),
        chosen(10, 170.0),
    ];
    let upright = Expect {
        max_tilt_deg: Some(90.0),
        on_plate: None,
    };
    let verdict = judge(GOAL.into(), &poses, upright, 1000).expect("the goal's pose is there");
    assert_eq!(
        verdict.status(),
        Status::Fail,
        "as is passes, so it was reachable"
    );
    assert_eq!(verdict.reachable_by.as_deref(), Some("as is"));

    let mut loose = pose("as is", Source::Hand, 100, 0.0);
    loose.printout.islands = 12;
    let poses = [
        loose,
        pose("over", Source::Hand, 10, 170.0),
        chosen(10, 170.0),
    ];
    let verdict = judge(GOAL.into(), &poses, upright, 1000).expect("the goal's pose is there");
    assert_eq!(
        verdict.status(),
        Status::Unreachable,
        "every pose that prints is upside down, and the one upright one is loose"
    );
}

#[test]
fn a_choice_past_the_time_budget_is_too_slow() {
    let mut slow = chosen(40, 30.0);
    slow.seconds = 30.0;
    let poses = [pose("as is", Source::Hand, 100, 0.0), slow];
    assert_eq!(broken(&poses), ["time"]);
}

#[test]
fn with_enough_pull_the_least_peel_fails_on_support_it_did_not_need() {
    let mut heavy = chosen(8, 30.0);
    heavy.printout.supports_ml = 2.0;
    let poses = [
        pose("as is", Source::Hand, 100, 0.0),
        pose("lean", Source::Hand, 5, 20.0),
        heavy,
    ];
    assert_eq!(
        broken(&poses),
        ["goal"],
        "8 N and 5 N both have enough; 2 ml against 0.5"
    );
    let spare = chosen(6, 30.0);
    let poses = [
        pose("as is", Source::Hand, 100, 0.0),
        pose("lean", Source::Hand, 5, 20.0),
        spare,
    ];
    assert!(
        broken(&poses).is_empty(),
        "0.6 ml is within a quarter and a strut of 0.5"
    );
}

#[test]
fn a_turn_past_enough_pull_does_not_stay_for_the_support_it_spares() {
    let mut lighter = chosen(11, 30.0);
    lighter.printout.supports_ml = 0.2;
    let poses = [pose("as is", Source::Hand, 5, 0.0), lighter];
    assert_eq!(
        broken(&poses),
        ["stays"],
        "11 N has not enough, so 0.2 ml against 0.5 does not pay for the turn"
    );
}

#[test]
fn a_turn_with_enough_pull_spares_support_against_the_lightest_way_as_drawn() {
    let mut heavy = pose("as is", Source::Hand, 5, 0.0);
    heavy.printout.supports_ml = 2.0;
    let mut turned = chosen(3, 30.0);
    turned.printout.supports_ml = 0.75;
    let poses = [heavy, pose("as is too", Source::Hand, 6, 0.0), turned];
    assert_eq!(
        broken(&poses),
        ["stays"],
        "both as drawn have enough; 0.75 ml spares nothing against 0.6"
    );
}

#[test]
fn a_tear_fails_a_goal_that_weighs_cups_but_not_the_shortest_print() {
    let mut torn = chosen(10, 30.0);
    torn.printout.peels = 1;
    torn.printout.layers = 50;
    let poses = [pose("as is", Source::Hand, 100, 0.0), torn];
    assert!(broken(&poses).contains(&"fatal"));
    assert!(
        broken_for(OrientGoal::ShortestPrint, &poses).is_empty(),
        "the limit is an estimate, and 50 layers beats 100"
    );
}

/// A pose reading `ml` of support and `layers` layers, pulling under enough.
fn trading(name: &str, source: Source, ml: f32, layers: usize, tilt_deg: f32) -> Pose {
    let mut pose = pose(name, source, 1, tilt_deg);
    pose.printout.supports_ml = ml;
    pose.printout.layers = layers;
    pose
}

fn auto_broken(poses: &[Pose]) -> Vec<&'static str> {
    judge(OrientPreset::Auto, poses, Expect::default(), 1000)
        .expect("the auto pose is among them")
        .broken()
}

const AUTO: Source = Source::Goal(OrientPreset::Auto);

#[test]
fn auto_passes_a_trade_no_sound_pose_beats_on_every_goal() {
    let poses = [
        trading("as is", Source::Hand, 1.0, 400, 0.0),
        trading("flat", Source::Hand, 3.0, 200, 90.0),
        trading("chosen", AUTO, 1.6, 300, 30.0),
    ];
    assert!(auto_broken(&poses).is_empty(), "{:?}", auto_broken(&poses));
}

#[test]
fn auto_fails_a_pose_beaten_on_every_goal() {
    let poses = [
        trading("as is", Source::Hand, 1.0, 300, 0.0),
        trading("chosen", AUTO, 1.6, 320, 30.0),
    ];
    assert_eq!(auto_broken(&poses), ["goal", "stays"]);
}

#[test]
fn auto_fails_a_trade_where_some_pose_is_good_on_every_goal() {
    let poses = [
        trading("cheap", Source::Hand, 1.0, 330, 10.0),
        trading("short", Source::Hand, 1.5, 300, 20.0),
        trading("all round", Source::Hand, 1.05, 310, 40.0),
        trading("chosen", AUTO, 1.4, 302, 30.0),
    ];
    assert_eq!(auto_broken(&poses), ["goal"]);
}

#[test]
fn auto_fails_a_goal_paid_past_three_times_its_best() {
    let poses = [
        trading("as is", Source::Hand, 0.5, 2000, 0.0),
        trading("chosen", AUTO, 5.0, 200, 30.0),
    ];
    assert_eq!(auto_broken(&poses), ["goal"], "5.5 ml against 1 ml");
}

#[test]
fn auto_fails_a_cup_and_a_tear_like_the_least_peel() {
    let mut chosen = trading("chosen", AUTO, 0.5, 200, 30.0);
    chosen.printout.peels = 1;
    let poses = [trading("as is", Source::Hand, 1.0, 300, 0.0), chosen];
    assert!(auto_broken(&poses).contains(&"fatal"));
}

#[test]
fn auto_stays_where_standing_as_drawn_is_good_on_every_goal() {
    let poses = [
        trading("as is", Source::Hand, 1.0, 300, 0.0),
        trading("chosen", AUTO, 1.0, 299, 30.0),
    ];
    assert_eq!(auto_broken(&poses), ["stays"]);
}
