//! What counts as a goal succeeding against the best pose measured for the same model.
//! Why each number is what it is: 

use core_engine::Printout;
use core_plate::{OrientGoal, OrientPreset};
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

/// How one preset's choice did.
#[derive(Debug, Clone)]
pub struct Verdict {
    pub goal: OrientPreset,
    pub checks: Vec<Check>,
    /// The goal's own reading; for the auto preset, its worst goal's reading as a multiple of
    /// that goal's best.
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

/// What every pose is held to: fewest cups and tears, then fewest islands and contacts.
/// Fewest supports leaves cups and tears to a drain hole; shortest print takes a tear.
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
        let weighs_cups = goal != OrientGoal::FewestSupports;
        let weighs_tears = goal != OrientGoal::ShortestPrint;
        Self::weighing(poses, weighs_cups, weighs_tears)
    }

    fn weighing(poses: &[Pose], weighs_cups: bool, weighs_tears: bool) -> Self {
        let any_fits = poses.iter().any(|pose| pose.fits);
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

/// Judges what `preset` chose for one model against every pose measured for it, and
/// whether any of them would have passed.
pub fn judge(
    preset: OrientPreset,
    poses: &[Pose],
    expect: Expect,
    faces: usize,
) -> Option<Verdict> {
    let chosen = poses
        .iter()
        .find(|pose| pose.source == Source::Goal(preset))?;
    let mut verdict = match preset {
        OrientPreset::Goal(goal) => judge_goal(goal, chosen, poses, expect),
        OrientPreset::Auto => judge_auto(chosen, poses, expect),
    };
    verdict.checks.push(time(chosen, faces));
    Some(verdict)
}

fn judge_goal(goal: OrientGoal, chosen: &Pose, poses: &[Pose], expect: Expect) -> Verdict {
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

    Verdict {
        goal: goal.into(),
        checks: held_to(chosen),
        reading: reading(goal, chosen),
        best,
        reachable_by,
    }
}

/// The judge's own bands for the auto preset (README, 2.3a): a tenth and the noise over
/// the best on every goal is as good as the best.
const AUTO_BAND: f64 = 0.10;
/// The most any goal may read as a multiple of its best for the balance to count as one:
/// three times the support or the time for another goal's sake is no longer a balance.
const AUTO_MOST: f64 = 3.0;
/// What the judge adds under each goal's reading before it is taken as a multiple: half a
/// millilitre of support, nothing past the enough pull, and thirty layers.
const AUTO_FLOOR: [f64; OrientGoal::ALL.len()] = [0.5, 0.0, 30.0];

/// Every goal's reading of `pose` as the judge counts them, enough pull and all.
fn readings(pose: &Pose) -> [f64; OrientGoal::ALL.len()] {
    OrientGoal::ALL.map(|goal| effective(goal, pose))
}

/// What the auto preset is weighed against: the best reading of every goal among `sound`.
struct Balance<'a> {
    sound: Vec<&'a Pose>,
    best: [f64; OrientGoal::ALL.len()],
}

impl<'a> Balance<'a> {
    fn of(sound: Vec<&'a Pose>) -> Option<Self> {
        let first = sound.first()?;
        let best = sound.iter().fold(readings(first), |best, pose| {
            let own = readings(pose);
            std::array::from_fn(|at| best[at].min(own[at]))
        });
        Some(Self { sound, best })
    }

    /// Each goal's reading of `pose` as a multiple of its best, floors added.
    fn multiples(&self, pose: &Pose) -> [f64; OrientGoal::ALL.len()] {
        let own = readings(pose);
        std::array::from_fn(|at| {
            (own[at] + AUTO_FLOOR[at]) / (self.best[at] + AUTO_FLOOR[at]).max(f64::EPSILON)
        })
    }

    fn worst(&self, pose: &Pose) -> f64 {
        self.multiples(pose).into_iter().fold(0.0, f64::max)
    }

    fn in_every_band(&self, pose: &Pose) -> bool {
        let own = readings(pose);
        OrientGoal::ALL
            .iter()
            .enumerate()
            .all(|(at, goal)| own[at] <= self.best[at] * (1.0 + AUTO_BAND) + noise(*goal))
    }

    /// A sound pose reading no worse than `pose` on any goal and better past the noise on one.
    fn beaten_by(&self, pose: &Pose) -> Option<&'a Pose> {
        let own = readings(pose);
        self.sound.iter().copied().find(|other| {
            let theirs = readings(other);
            let mut better = false;
            for (at, goal) in OrientGoal::ALL.iter().enumerate() {
                if theirs[at] > own[at] {
                    return false;
                }
                better |= theirs[at] < own[at] - noise(*goal);
            }
            better
        })
    }
}

/// Judges the auto preset: sound, good on every goal where some pose is, beaten on all of
/// them by no sound pose, and no goal past [`AUTO_MOST`] of its best.
fn judge_auto(chosen: &Pose, poses: &[Pose], expect: Expect) -> Verdict {
    let floor = Floor::weighing(poses, true, true);
    let acceptable = |pose: &Pose| expected(pose, expect).iter().all(|check| check.passed);
    let balance = Balance::of(
        poses
            .iter()
            .filter(|pose| floor.sound(pose) && acceptable(pose))
            .collect(),
    );
    let held_to = |pose: &Pose| held_to_auto(pose, poses, &floor, balance.as_ref(), expect);
    let reachable_by = poses
        .iter()
        .find(|pose| held_to(pose).iter().all(|check| check.passed))
        .map(|pose| pose.name.clone());
    let best = balance.as_ref().and_then(|balance| {
        balance
            .sound
            .iter()
            .min_by(|a, b| balance.worst(a).total_cmp(&balance.worst(b)))
            .map(|pose| (pose.name.clone(), balance.worst(pose)))
    });
    Verdict {
        goal: OrientPreset::Auto,
        checks: held_to(chosen),
        reading: balance
            .as_ref()
            .map_or(0.0, |balance| balance.worst(chosen)),
        best,
        reachable_by,
    }
}

fn held_to_auto(
    pose: &Pose,
    poses: &[Pose],
    floor: &Floor,
    balance: Option<&Balance>,
    expect: Expect,
) -> Vec<Check> {
    let mut checks = floor.prints(pose).to_vec();
    if let Some(balance) = balance {
        let sound = floor.sound(pose);
        let all_round = balance
            .sound
            .iter()
            .find(|other| balance.in_every_band(other))
            .filter(|_| !balance.in_every_band(pose));
        let beaten_by = balance.beaten_by(pose);
        let multiples = balance.multiples(pose);
        let lopsided = multiples.iter().any(|multiple| *multiple > AUTO_MOST);
        let [ml, n, layers] = multiples;
        let mut detail = format!("supports ×{ml:.2}, pull ×{n:.2}, layers ×{layers:.2}");
        if !sound {
            detail.push_str("; not sound, so no win");
        }
        if let Some(other) = all_round {
            detail.push_str(&format!("; {} is good on every goal", other.name));
        }
        if let Some(other) = beaten_by {
            detail.push_str(&format!("; beaten on every goal by {}", other.name));
        }
        if lopsided {
            detail.push_str(&format!("; a goal past ×{AUTO_MOST:.0} its best"));
        }
        checks.push(check(
            Rule::Goal,
            sound && all_round.is_none() && beaten_by.is_none() && !lopsided,
            detail,
        ));
        checks.push(stays_auto(pose, poses, floor, balance));
    }
    checks.extend(expected(pose, expect));
    checks
}

/// A turn is only worth it when standing as drawn is not good on every goal, or the turn
/// wins [`turn_cost`] over it on one goal and loses past the noise on none.
fn stays_auto(chosen: &Pose, poses: &[Pose], floor: &Floor, balance: &Balance) -> Check {
    let drawn = poses.iter().find(|pose| {
        pose.tilt_deg < SETTLED_DEG && floor.sound(pose) && balance.in_every_band(pose)
    });
    let passed = match drawn {
        Some(drawn) if chosen.tilt_deg >= SETTLED_DEG => {
            let (own, theirs) = (readings(chosen), readings(drawn));
            let goals = OrientGoal::ALL.iter().enumerate();
            let won = goals
                .clone()
                .any(|(at, goal)| own[at] < theirs[at] - turn_cost(*goal, theirs[at]));
            let lost = goals
                .clone()
                .any(|(at, goal)| own[at] > theirs[at] + noise(*goal));
            won && !lost
        }
        _ => true,
    };
    let detail = drawn.map_or_else(
        || "standing as drawn is not good on every goal".to_owned(),
        |drawn| {
            format!(
                "turned {:.0}° from {}, good on every goal",
                chosen.tilt_deg, drawn.name
            )
        },
    );
    check(Rule::Stays, passed, detail)
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
mod tests;
