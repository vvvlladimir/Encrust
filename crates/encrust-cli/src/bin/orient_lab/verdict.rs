//! What counts as a goal succeeding: the rules every goal's choice is held to against the
//! best pose measured for the same model, whoever found it. The rules and why each number
//! is what it is live in `local-docs/orientation/README.md`; nothing here is a reference
//! answer, only the current bar.

use core_engine::Printout;
use core_plate::OrientGoal;
use serde::Deserialize;

use crate::poses::{Pose, Source};

/// Islands and hanging contacts a pose may have over the fewest of any sound pose: a
/// quarter and two, since an island more or less is the grain of the support grid.
const HELD_BAND: f64 = 1.25;
const HELD_SLACK: f64 = 2.0;

/// How far over the best known reading a goal's choice may read; the fewest supports takes
/// the least (ADR 0279).
fn goal_band(goal: OrientGoal) -> f64 {
    if goal == OrientGoal::FewestSupports {
        0.0
    } else {
        0.10
    }
}

/// What a turn has to win over standing as drawn, when standing as drawn is good enough:
/// a twentieth and the reading's noise, or anything at all for the fewest supports.
fn turn_cost(goal: OrientGoal, drawn: f64) -> f64 {
    if goal == OrientGoal::FewestSupports {
        0.0
    } else {
        0.05 * drawn + noise(goal)
    }
}

/// The layers a full print is counted alike to, the judge's own (README, 2.3a).
const LAYER_NOISE: f64 = 5.0;

/// What of `goal`'s [`reading`] is noise.
fn noise(goal: OrientGoal) -> f64 {
    if goal == OrientGoal::ShortestPrint {
        LAYER_NOISE
    } else {
        Printout::noise(goal)
    }
}

/// `goal`'s reading of `pose` as the judge counts it: layers for the shortest print.
fn reading(goal: OrientGoal, pose: &Pose) -> f64 {
    if goal == OrientGoal::ShortestPrint {
        pose.printout.layers as f64
    } else {
        pose.printout.reading(goal)
    }
}

/// The pull the least peel has enough at, newtons, the judge's own (README, 2.3a), and the
/// support past the least such pose's it may then take, a quarter and a strut.
const PEEL_ENOUGH_N: f64 = 10.0;
const SPARE_BAND: f64 = 1.25;
const SPARE_SLACK_ML: f64 = 0.05;

/// `goal`'s reading of `pose`, no lower than what the goal counts as enough.
fn effective(goal: OrientGoal, pose: &Pose) -> f64 {
    let reading = reading(goal, pose);
    if goal == OrientGoal::LeastPeel {
        reading.max(PEEL_ENOUGH_N)
    } else {
        reading
    }
}

/// The support a pose takes, millilitres.
fn support_ml(pose: &Pose) -> f64 {
    pose.printout.reading(OrientGoal::FewestSupports)
}

/// A tilt under this is standing as drawn, degrees.
const SETTLED_DEG: f32 = 2.0;

/// Seconds a goal may take, a million faces, and never less than the floor: three seconds
/// is a wait a person sits through, and part of the cost follows the volume, not the faces.
const SECONDS_PER_MILLION_FACES: f64 = 3.0;
const SECONDS_FLOOR: f64 = 3.0;

/// One rule of success. The name is what reports and baselines carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    /// No more suction cups and tearing pulls than the soundest pose.
    Fatal,
    /// No more islands and hanging contacts than the soundest pose, within the band.
    Held,
    /// A pose set on the plate leaves nothing hanging.
    Seated,
    /// It fits the build volume whenever some pose does.
    Fits,
    /// Its own reading within the band of the best sound pose.
    Goal,
    /// Not turned when standing as drawn was good enough.
    Stays,
    /// What a person wrote down for this model.
    Expected,
    /// Within the time budget.
    Time,
}

impl Rule {
    pub fn name(self) -> &'static str {
        match self {
            Self::Fatal => "fatal",
            Self::Held => "held",
            Self::Seated => "seated",
            Self::Fits => "fits",
            Self::Goal => "goal",
            Self::Stays => "stays",
            Self::Expected => "expected",
            Self::Time => "time",
        }
    }
}

/// What a person expects of one model under one goal. Every field is optional.
#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Expect {
    /// The most the model's own up may lean, degrees: 90 forbids upside down. Whether a
    /// model has an up at all is a person's call, so no rule of every goal makes it.
    pub max_tilt_deg: Option<f32>,
    /// Whether it should end up set straight on the plate.
    pub on_plate: Option<bool>,
}

/// One rule applied to the chosen pose.
#[derive(Debug, Clone, PartialEq)]
pub struct Check {
    pub rule: Rule,
    pub passed: bool,
    pub detail: String,
}

/// How one goal's choice did.
#[derive(Debug, Clone)]
pub struct Verdict {
    pub goal: OrientGoal,
    pub checks: Vec<Check>,
    pub reading: f64,
    /// The best reading of any sound pose, and which pose read it.
    pub best: Option<(String, f64)>,
    /// A measured pose that would have passed every rule but time, had the goal chosen
    /// it, or `None` when none would: success is out of reach of every pose known.
    pub reachable_by: Option<String>,
}

/// Where a goal stands on one model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Pass,
    Fail,
    /// No pose measured for the model would pass: the model, not the goal, is in question.
    Unreachable,
}

impl Status {
    pub fn name(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Unreachable => "unreachable",
        }
    }
}

impl Verdict {
    pub fn passed(&self) -> bool {
        self.checks.iter().all(|check| check.passed)
    }

    pub fn status(&self) -> Status {
        if self.passed() {
            Status::Pass
        } else if self.reachable_by.is_some() {
            Status::Fail
        } else {
            Status::Unreachable
        }
    }

    /// The names of the rules it broke.
    pub fn broken(&self) -> Vec<&'static str> {
        self.checks
            .iter()
            .filter(|check| !check.passed)
            .map(|check| check.rule.name())
            .collect()
    }
}

/// What every pose of a model is held to: the fewest cups and tears, and the fewest
/// islands and hanging contacts among those, of all poses that fit. The fewest supports
/// leaves cups and tears to a drain hole, and islands and contacts with no room as
/// placement leaves them, so for it those count for nothing; the shortest print takes a
/// tear, the limit being an estimate.
struct Floor {
    any_fits: bool,
    /// Whether cups, tears, islands and hanging contacts count.
    weighs_cups: bool,
    /// Whether, those counting, a pull past the limit does too.
    weighs_tears: bool,
    fatal: usize,
    held_limit: f64,
}

impl Floor {
    fn of(poses: &[Pose], goal: OrientGoal) -> Self {
        let any_fits = poses.iter().any(|pose| pose.fits);
        let weighs_cups = goal != OrientGoal::FewestSupports;
        let weighs_tears = goal != OrientGoal::ShortestPrint;
        let fatal_of = |pose: &Pose| fatal(pose, weighs_cups, weighs_tears);
        let fitting = || poses.iter().filter(move |pose| pose.fits || !any_fits);
        let fatal = fitting().map(fatal_of).min().unwrap_or(0);
        let unheld = |pose: &Pose| {
            if weighs_cups {
                pose.printout.islands + pose.printout.hanging
            } else {
                0
            }
        };
        let held = fitting()
            .filter(|pose| fatal_of(pose) == fatal)
            .map(unheld)
            .min()
            .unwrap_or(0);
        Self {
            any_fits,
            weighs_cups,
            weighs_tears,
            fatal,
            held_limit: held as f64 * HELD_BAND + HELD_SLACK,
        }
    }

    fn unheld(&self, pose: &Pose) -> usize {
        if self.weighs_cups {
            pose.printout.islands + pose.printout.hanging
        } else {
            0
        }
    }

    fn fatal(&self, pose: &Pose) -> usize {
        fatal(pose, self.weighs_cups, self.weighs_tears)
    }

    /// The four rules about whether the pose prints at all, in order.
    fn prints(&self, pose: &Pose) -> [Check; 4] {
        let o = &pose.printout;
        [
            check(
                Rule::Fatal,
                self.fatal(pose) <= self.fatal,
                format!(
                    "{} cups and counted tears, fewest {}",
                    self.fatal(pose),
                    self.fatal
                ),
            ),
            check(
                Rule::Held,
                self.unheld(pose) as f64 <= self.held_limit,
                format!(
                    "{} islands and hanging, at most {:.1}",
                    self.unheld(pose),
                    self.held_limit
                ),
            ),
            check(
                Rule::Seated,
                !pose.on_plate || (sits(pose) && (o.hanging == 0 || !self.weighs_cups)),
                if sits(pose) {
                    format!("{} hanging on the plate", o.hanging)
                } else {
                    "balanced on an edge or a point".to_owned()
                },
            ),
            check(
                Rule::Fits,
                pose.fits || !self.any_fits,
                "the build volume".to_owned(),
            ),
        ]
    }

    fn sound(&self, pose: &Pose) -> bool {
        self.prints(pose).iter().all(|check| check.passed)
    }
}

/// The cups and, where they count, the tears of `pose`, where cups count at all.
fn fatal(pose: &Pose, weighs_cups: bool, weighs_tears: bool) -> usize {
    match (weighs_cups, weighs_tears) {
        (false, _) => 0,
        (true, false) => pose.printout.cups,
        (true, true) => pose.printout.fatal(),
    }
}

/// Whether a pose on the plate stands there rather than balances: on a flat face, or on
/// supports that hold it.
fn sits(pose: &Pose) -> bool {
    pose.sits || pose.printout.contacts > 0
}

fn check(rule: Rule, passed: bool, detail: String) -> Check {
    Check {
        rule,
        passed,
        detail,
    }
}

/// Judges what `goal` chose for one model against every pose measured for it, and
/// whether any of them would have passed.
pub fn judge(goal: OrientGoal, poses: &[Pose], expect: Expect, faces: usize) -> Option<Verdict> {
    let chosen = poses
        .iter()
        .find(|pose| pose.source == Source::Goal(goal))?;
    let floor = Floor::of(poses, goal);
    let effective_of = |pose: &Pose| effective(goal, pose);
    // The best is sought among the poses a person would accept at all.
    let acceptable = |pose: &Pose| expected(pose, expect).iter().all(|check| check.passed);
    let best = poses
        .iter()
        .filter(|pose| floor.sound(pose) && acceptable(pose))
        .min_by(|a, b| {
            effective_of(a)
                .total_cmp(&effective_of(b))
                .then(support_ml(a).total_cmp(&support_ml(b)))
        })
        .map(|pose| (pose.name.clone(), effective_of(pose)));
    let held_to = |pose: &Pose| held_to(goal, pose, poses, &floor, best.as_ref(), expect);
    let reachable_by = poses
        .iter()
        .find(|pose| held_to(pose).iter().all(|check| check.passed))
        .map(|pose| pose.name.clone());

    let mut checks = held_to(chosen);
    checks.push(time(chosen, faces));
    Some(Verdict {
        goal,
        checks,
        reading: reading(goal, chosen),
        best,
        reachable_by,
    })
}

/// Every rule but time applied to `pose` as if `goal` had chosen it: time is the search's
/// own, and says nothing of a pose.
fn held_to(
    goal: OrientGoal,
    pose: &Pose,
    poses: &[Pose],
    floor: &Floor,
    best: Option<&(String, f64)>,
    expect: Expect,
) -> Vec<Check> {
    let chosen = pose;
    let mut checks = floor.prints(chosen).to_vec();
    let own = effective(goal, chosen);
    if let Some((name, best_reading)) = best {
        let limit = best_reading * (1.0 + goal_band(goal)) + noise(goal);
        // A reading bought by failing the print wins nothing.
        let sound = floor.sound(chosen);
        let unsound = if sound { "" } else { "not sound, so no win: " };
        let spare = spare(goal, chosen, poses, floor);
        checks.push(check(
            Rule::Goal,
            sound && own <= limit && spare.is_none(),
            format!(
                "{unsound}{:.1}, best {best_reading:.1} by {name}{}",
                reading(goal, chosen),
                spare.unwrap_or_default()
            ),
        ));
        checks.push(stays(goal, chosen, poses, floor, limit));
    }
    checks.extend(expected(chosen, expect));
    checks
}

/// Where the least peel has enough, what it took in support past the least any sound pose
/// with enough takes, as the reason it fails; `None` when it took no more.
fn spare(goal: OrientGoal, chosen: &Pose, poses: &[Pose], floor: &Floor) -> Option<String> {
    if goal != OrientGoal::LeastPeel || effective(goal, chosen) > PEEL_ENOUGH_N {
        return None;
    }
    let (name, least) = poses
        .iter()
        .filter(|pose| floor.sound(pose) && effective(goal, pose) <= PEEL_ENOUGH_N)
        .map(|pose| (pose.name.as_str(), support_ml(pose)))
        .min_by(|a, b| a.1.total_cmp(&b.1))?;
    let own = support_ml(chosen);
    (own > least * SPARE_BAND + SPARE_SLACK_ML)
        .then(|| format!("; enough, but {own:.2} ml of support against {least:.2} by {name}"))
}

/// A turn is only worth it when standing as drawn is out of the band, or the turn wins
/// [`turn_cost`] over it; where both have enough pull, it has to spare that in support.
fn stays(goal: OrientGoal, chosen: &Pose, poses: &[Pose], floor: &Floor, limit: f64) -> Check {
    let reading = |pose: &Pose| effective(goal, pose);
    let drawn_pose = poses
        .iter()
        .filter(|pose| pose.tilt_deg < SETTLED_DEG && floor.sound(pose))
        .min_by(|a, b| {
            reading(a)
                .total_cmp(&reading(b))
                .then(support_ml(a).total_cmp(&support_ml(b)))
        })
        .filter(|pose| reading(pose) <= limit);
    let as_drawn = drawn_pose.map(reading);
    let passed = match drawn_pose {
        Some(drawn) if chosen.tilt_deg >= SETTLED_DEG => {
            let won = reading(chosen) <= reading(drawn) - turn_cost(goal, reading(drawn));
            let enough = goal == OrientGoal::LeastPeel
                && reading(drawn).max(reading(chosen)) <= PEEL_ENOUGH_N;
            let spared = support_ml(chosen)
                <= support_ml(drawn) - turn_cost(OrientGoal::FewestSupports, support_ml(drawn));
            won || (enough && spared)
        }
        _ => true,
    };
    let detail = as_drawn.map_or_else(
        || "standing as drawn is not good enough".to_owned(),
        |drawn| format!("turned {:.0}° against {drawn:.1} as drawn", chosen.tilt_deg),
    );
    check(Rule::Stays, passed, detail)
}

fn expected(chosen: &Pose, expect: Expect) -> Vec<Check> {
    let mut checks = Vec::new();
    if let Some(max_deg) = expect.max_tilt_deg {
        checks.push(check(
            Rule::Expected,
            chosen.tilt_deg <= max_deg,
            format!("tilted {:.0}°, at most {max_deg:.0}°", chosen.tilt_deg),
        ));
    }
    if let Some(on_plate) = expect.on_plate {
        checks.push(check(
            Rule::Expected,
            chosen.on_plate == on_plate,
            format!("on the plate {}, expected {on_plate}", chosen.on_plate),
        ));
    }
    checks
}

fn time(chosen: &Pose, faces: usize) -> Check {
    let budget = (SECONDS_PER_MILLION_FACES * faces as f64 / 1e6).max(SECONDS_FLOOR);
    check(
        Rule::Time,
        chosen.seconds <= budget,
        format!("{:.1} s, budget {budget:.1} s", chosen.seconds),
    )
}

#[cfg(test)]
mod tests {
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
        pose("chosen", Source::Goal(GOAL), contacts, tilt_deg)
    }

    fn broken(poses: &[Pose]) -> Vec<&'static str> {
        broken_for(GOAL, poses)
    }

    /// What `goal` breaks having chosen the pose called "chosen".
    fn broken_for(goal: OrientGoal, poses: &[Pose]) -> Vec<&'static str> {
        let mut poses = poses.to_vec();
        for pose in poses.iter_mut().filter(|pose| pose.name == "chosen") {
            pose.source = Source::Goal(goal);
        }
        judge(goal, &poses, Expect::default(), 1000)
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
        let verdict = judge(GOAL, &poses, expect, 1000).expect("the goal's pose is there");
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
        let verdict = judge(GOAL, &poses, upright, 1000).expect("the goal's pose is there");
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
        let verdict = judge(GOAL, &poses, upright, 1000).expect("the goal's pose is there");
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
}
