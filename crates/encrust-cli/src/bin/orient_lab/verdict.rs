//! What counts as a goal succeeding: the rules every goal's choice is held to against the
//! best pose measured for the same model, whoever found it. The rules and why each number
//! is what it is live in `local-docs/orientation/README.md`; nothing here is a reference
//! answer, only the current bar.

use core_plate::OrientGoal;
use serde::Deserialize;

use crate::poses::{Pose, Source};

/// Islands and hanging contacts a pose may have over the fewest of any sound pose: a
/// quarter and two, since an island more or less is the grain of the support grid.
const HELD_BAND: f64 = 1.25;
const HELD_SLACK: f64 = 2.0;

/// How far over the best known reading a goal's choice may read.
const GOAL_BAND: f64 = 0.10;

/// What a turn has to win over standing as drawn, when standing as drawn is good enough.
const TURN_GAIN: f64 = 0.05;

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

/// What a goal's own reading may be off by and still be the same: a contact, a newton's
/// half, a few layers.
fn slack(goal: OrientGoal) -> f64 {
    match goal {
        OrientGoal::FewestSupports | OrientGoal::CleanTop => 2.0,
        OrientGoal::LeastPeel => 0.5,
        OrientGoal::ShortestPrint => 5.0,
    }
}

fn unheld(pose: &Pose) -> usize {
    pose.printout.islands + pose.printout.hanging
}

/// What every pose of a model is held to: the fewest cups and tears, and the fewest
/// islands and hanging contacts among those, of all poses that fit.
struct Floor {
    any_fits: bool,
    fatal: usize,
    held_limit: f64,
}

impl Floor {
    fn of(poses: &[Pose]) -> Self {
        let any_fits = poses.iter().any(|pose| pose.fits);
        let fitting = || poses.iter().filter(move |pose| pose.fits || !any_fits);
        let fatal = fitting()
            .map(|pose| pose.printout.fatal())
            .min()
            .unwrap_or(0);
        let held = fitting()
            .filter(|pose| pose.printout.fatal() == fatal)
            .map(unheld)
            .min()
            .unwrap_or(0);
        Self {
            any_fits,
            fatal,
            held_limit: held as f64 * HELD_BAND + HELD_SLACK,
        }
    }

    /// The four rules about whether the pose prints at all, in order.
    fn prints(&self, pose: &Pose) -> [Check; 4] {
        let o = &pose.printout;
        [
            check(
                Rule::Fatal,
                o.fatal() <= self.fatal,
                format!("{} cups and tears, fewest {}", o.fatal(), self.fatal),
            ),
            check(
                Rule::Held,
                unheld(pose) as f64 <= self.held_limit,
                format!(
                    "{} islands and hanging, at most {:.1}",
                    unheld(pose),
                    self.held_limit
                ),
            ),
            check(
                Rule::Seated,
                !pose.on_plate || o.hanging == 0,
                format!("{} hanging on the plate", o.hanging),
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
    let floor = Floor::of(poses);
    let reading = |pose: &Pose| pose.printout.reading(goal);
    // The best is sought among the poses a person would accept at all.
    let acceptable = |pose: &Pose| expected(pose, expect).iter().all(|check| check.passed);
    let best = poses
        .iter()
        .filter(|pose| floor.sound(pose) && acceptable(pose))
        .min_by(|a, b| reading(a).total_cmp(&reading(b)))
        .map(|pose| (pose.name.clone(), reading(pose)));
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
        reading: reading(chosen),
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
    let own = chosen.printout.reading(goal);
    if let Some((name, best_reading)) = best {
        let limit = best_reading * (1.0 + GOAL_BAND) + slack(goal);
        // A reading bought by failing the print wins nothing.
        let sound = floor.sound(chosen);
        let unsound = if sound { "" } else { "not sound, so no win: " };
        checks.push(check(
            Rule::Goal,
            sound && own <= limit,
            format!("{unsound}{own:.1}, best {best_reading:.1} by {name}"),
        ));
        checks.push(stays(goal, chosen, poses, floor, limit));
    }
    checks.extend(expected(chosen, expect));
    checks
}

/// A turn is only worth it when standing as drawn is out of the band, or the turn wins
/// `TURN_GAIN` over it.
fn stays(goal: OrientGoal, chosen: &Pose, poses: &[Pose], floor: &Floor, limit: f64) -> Check {
    let reading = |pose: &Pose| pose.printout.reading(goal);
    let as_drawn = poses
        .iter()
        .filter(|pose| pose.tilt_deg < SETTLED_DEG && floor.sound(pose))
        .map(reading)
        .min_by(f64::total_cmp)
        .filter(|drawn| *drawn <= limit);
    let passed = match as_drawn {
        Some(drawn) if chosen.tilt_deg >= SETTLED_DEG => {
            reading(chosen) + slack(goal) <= drawn * (1.0 - TURN_GAIN)
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

    const GOAL: OrientGoal = OrientGoal::FewestSupports;

    fn printout(contacts: usize) -> Printout {
        Printout {
            contacts,
            hanging: 0,
            top_contacts: 0,
            layers: 100,
            pull_n: 1.0,
            growth_mm2: 0.0,
            islands: 0,
            cups: 0,
            peels: 0,
            resin_ml: 1.0,
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
            seconds: 0.1,
            trace: crate::poses::Trace::default(),
        }
    }

    fn chosen(contacts: usize, tilt_deg: f32) -> Pose {
        pose("chosen", Source::Goal(GOAL), contacts, tilt_deg)
    }

    fn broken(poses: &[Pose]) -> Vec<&'static str> {
        judge(GOAL, poses, Expect::default(), 1000)
            .expect("the goal's pose is among them")
            .broken()
    }

    #[test]
    fn the_best_pose_breaks_nothing() {
        let poses = [pose("as is", Source::Hand, 100, 0.0), chosen(40, 30.0)];
        assert!(broken(&poses).is_empty());
    }

    #[test]
    fn a_cup_the_soundest_pose_avoids_is_fatal() {
        let mut cupped = chosen(10, 30.0);
        cupped.printout.cups = 1;
        let poses = [pose("as is", Source::Hand, 100, 0.0), cupped];
        assert_eq!(broken(&poses), ["fatal", "goal"]);
    }

    #[test]
    fn islands_past_the_band_of_the_soundest_pose_are_not_held() {
        let mut loose = chosen(10, 30.0);
        loose.printout.islands = 12;
        let poses = [pose("as is", Source::Hand, 100, 0.0), loose];
        assert_eq!(broken(&poses), ["held", "goal"]);
    }

    #[test]
    fn a_pose_on_the_plate_with_something_hanging_is_not_seated() {
        let mut bare = chosen(0, 30.0);
        bare.on_plate = true;
        bare.printout.hanging = 1;
        let poses = [pose("as is", Source::Hand, 100, 0.0), bare];
        assert!(broken(&poses).contains(&"seated"));
    }

    #[test]
    fn a_pose_out_of_the_volume_fails_when_another_fits() {
        let mut big = chosen(10, 30.0);
        big.fits = false;
        let poses = [pose("as is", Source::Hand, 100, 0.0), big];
        assert!(broken(&poses).contains(&"fits"));
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
    fn a_turn_that_barely_beats_standing_as_drawn_is_needless() {
        let poses = [pose("as is", Source::Hand, 100, 0.0), chosen(97, 30.0)];
        assert_eq!(broken(&poses), ["stays"]);
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

        let mut cupped = pose("as is", Source::Hand, 100, 0.0);
        cupped.printout.cups = 1;
        let poses = [
            cupped,
            pose("over", Source::Hand, 10, 170.0),
            chosen(10, 170.0),
        ];
        let verdict = judge(GOAL, &poses, upright, 1000).expect("the goal's pose is there");
        assert_eq!(
            verdict.status(),
            Status::Unreachable,
            "every pose that prints is upside down, and the one upright one cups"
        );
    }

    #[test]
    fn a_choice_past_the_time_budget_is_too_slow() {
        let mut slow = chosen(40, 30.0);
        slow.seconds = 30.0;
        let poses = [pose("as is", Source::Hand, 100, 0.0), slow];
        assert_eq!(broken(&poses), ["time"]);
    }
}
