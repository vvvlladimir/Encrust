//! What a run found, as a table to read and as JSON to keep, and how it compares with a
//! run kept before.

use std::path::Path;

use anyhow::{Context, Result};
use core_engine::Printout;
use core_geometry::Quat;
use core_plate::OrientGoal;
use serde::{Deserialize, Serialize};

use crate::poses::{Pose, Source};
use crate::verdict::Verdict;

/// Bumped when a field changes meaning, so an old baseline is refused rather than misread.
const SCHEMA: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub schema: u32,
    pub printer: String,
    pub support: String,
    pub oracle: usize,
    pub models: Vec<ModelReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelReport {
    pub name: String,
    pub faces: usize,
    /// Why the model was not benched, or `None` when it was.
    pub skipped: Option<String>,
    pub poses: Vec<PoseRow>,
    pub goals: Vec<GoalRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoseRow {
    pub name: String,
    pub source: String,
    /// The turn as a quaternion, x, y, z, w, so a pose can be stood up again by hand.
    pub rotation: [f32; 4],
    pub tilt_deg: f32,
    pub on_plate: bool,
    pub fits: bool,
    pub contacts: usize,
    pub hanging: usize,
    pub top_contacts: usize,
    pub layers: usize,
    pub pull_n: f64,
    pub growth_mm2: f32,
    pub islands: usize,
    pub cups: usize,
    pub peels: usize,
    pub resin_ml: f32,
    pub seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalRow {
    pub goal: String,
    pub passed: bool,
    /// The rules the choice broke, by name.
    pub broken: Vec<String>,
    pub reading: f64,
    pub best_pose: Option<String>,
    pub best_reading: Option<f64>,
    pub seconds: f64,
}

/// The name a goal goes by in reports and corpus files.
pub fn slug(goal: OrientGoal) -> &'static str {
    match goal {
        OrientGoal::FewestSupports => "fewest-supports",
        OrientGoal::LeastPeel => "least-peel",
        OrientGoal::ShortestPrint => "shortest-print",
        OrientGoal::CleanTop => "clean-top",
    }
}

/// The goal a slug names, or `None` for a slug no goal goes by.
fn goal_of(slug_text: &str) -> Option<OrientGoal> {
    OrientGoal::ALL
        .into_iter()
        .find(|goal| slug(*goal) == slug_text)
}

impl PoseRow {
    /// The pose as it was measured, to be judged again by rules that have changed since.
    pub fn pose(&self) -> Result<Pose> {
        let source = match self.source.as_str() {
            "hand" => Source::Hand,
            "oracle" => Source::Oracle,
            other => {
                Source::Goal(goal_of(other).with_context(|| format!("no goal is called {other}"))?)
            }
        };
        Ok(Pose {
            name: self.name.clone(),
            source,
            rotation: Quat::from_array(self.rotation),
            on_plate: self.on_plate,
            printout: Printout {
                contacts: self.contacts,
                hanging: self.hanging,
                top_contacts: self.top_contacts,
                layers: self.layers,
                pull_n: self.pull_n,
                growth_mm2: self.growth_mm2,
                islands: self.islands,
                cups: self.cups,
                peels: self.peels,
                resin_ml: self.resin_ml,
            },
            tilt_deg: self.tilt_deg,
            fits: self.fits,
            seconds: self.seconds,
        })
    }

    pub fn of(pose: &Pose) -> Self {
        let o = &pose.printout;
        Self {
            name: pose.name.clone(),
            source: match pose.source {
                Source::Hand => "hand".to_owned(),
                Source::Goal(goal) => slug(goal).to_owned(),
                Source::Oracle => "oracle".to_owned(),
            },
            rotation: pose.rotation.to_array(),
            tilt_deg: pose.tilt_deg,
            on_plate: pose.on_plate,
            fits: pose.fits,
            contacts: o.contacts,
            hanging: o.hanging,
            top_contacts: o.top_contacts,
            layers: o.layers,
            pull_n: o.pull_n,
            growth_mm2: o.growth_mm2,
            islands: o.islands,
            cups: o.cups,
            peels: o.peels,
            resin_ml: o.resin_ml,
            seconds: pose.seconds,
        }
    }
}

impl GoalRow {
    pub fn of(verdict: &Verdict, seconds: f64) -> Self {
        Self {
            goal: slug(verdict.goal).to_owned(),
            passed: verdict.passed(),
            broken: verdict.broken().into_iter().map(str::to_owned).collect(),
            reading: verdict.reading,
            best_pose: verdict.best.as_ref().map(|(name, _)| name.clone()),
            best_reading: verdict.best.as_ref().map(|(_, reading)| *reading),
            seconds,
        }
    }
}

impl Report {
    pub fn new(printer: &str, support: &str, oracle: usize) -> Self {
        Self {
            schema: SCHEMA,
            printer: printer.to_owned(),
            support: support.to_owned(),
            oracle,
            models: Vec::new(),
        }
    }

    pub fn write(&self, path: &Path) -> Result<()> {
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(path, text).with_context(|| format!("cannot write {}", path.display()))
    }

    pub fn read(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read {}", path.display()))?;
        let report: Self = serde_json::from_str(&text)
            .with_context(|| format!("{} is not an orient-lab report", path.display()))?;
        anyhow::ensure!(
            report.schema == SCHEMA,
            "{} is schema {}, this run writes {SCHEMA}",
            path.display(),
            report.schema
        );
        Ok(report)
    }

    /// Goals passed and goals judged.
    pub fn score(&self) -> (usize, usize) {
        let goals = self.models.iter().flat_map(|model| &model.goals);
        let judged = goals.clone().count();
        (goals.filter(|goal| goal.passed).count(), judged)
    }
}

/// Prints one model's poses and verdicts as markdown.
pub fn print_model(model: &ModelReport, verdicts: &[Verdict]) {
    println!("## {} — {} faces\n", model.name, model.faces);
    println!(
        "| pose | tilt ° | plate | fits | contacts | hanging | on top | layers | pull N | islands | cups | peels | resin ml | s |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|");
    for p in &model.poses {
        println!(
            "| {} | {:.0} | {} | {} | {} | {} | {} | {} | {:.1} | {} | {} | {} | {:.1} | {:.1} |",
            p.name,
            p.tilt_deg,
            if p.on_plate { "yes" } else { "—" },
            if p.fits { "yes" } else { "NO" },
            p.contacts,
            p.hanging,
            p.top_contacts,
            p.layers,
            p.pull_n,
            p.islands,
            p.cups,
            p.peels,
            p.resin_ml,
            p.seconds,
        );
    }
    println!();
    for verdict in verdicts {
        let call = if verdict.passed() { "pass" } else { "FAIL" };
        println!("- {}: **{call}**", verdict.goal.label());
        for check in verdict.checks.iter().filter(|check| !check.passed) {
            println!("  - {}: {}", check.rule.name(), check.detail);
        }
    }
    println!();
}

/// How one goal of one model moved between two runs.
#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    pub model: String,
    pub goal: String,
    pub was: Option<bool>,
    pub now: bool,
    pub reading: (Option<f64>, f64),
}

impl Change {
    /// A goal that passed before and fails now.
    pub fn regressed(&self) -> bool {
        self.was == Some(true) && !self.now
    }

    fn label(&self) -> &'static str {
        match (self.was, self.now) {
            (Some(true), false) => "REGRESSED",
            (Some(false), true) => "fixed",
            (None, _) => "new",
            _ => "same",
        }
    }
}

/// Every goal of `now` set beside the same goal of the same model in `was`.
pub fn compare(was: &Report, now: &Report) -> Vec<Change> {
    let before = |model: &str, goal: &str| {
        was.models
            .iter()
            .find(|m| m.name == model)
            .and_then(|m| m.goals.iter().find(|g| g.goal == goal))
    };
    now.models
        .iter()
        .flat_map(|model| {
            model.goals.iter().map(move |goal| {
                let old = before(&model.name, &goal.goal);
                Change {
                    model: model.name.clone(),
                    goal: goal.goal.clone(),
                    was: old.map(|old| old.passed),
                    now: goal.passed,
                    reading: (old.map(|old| old.reading), goal.reading),
                }
            })
        })
        .collect()
}

/// Prints the changes that are not `same`, then a count of each kind.
pub fn print_changes(changes: &[Change]) {
    println!("## Against the baseline\n");
    println!("| model | goal | change | reading was | reading now |");
    println!("|---|---|---|---|---|");
    for change in changes.iter().filter(|change| change.label() != "same") {
        let was = change
            .reading
            .0
            .map_or("—".to_owned(), |was| format!("{was:.1}"));
        println!(
            "| {} | {} | {} | {was} | {:.1} |",
            change.model,
            change.goal,
            change.label(),
            change.reading.1
        );
    }
    let count = |label| changes.iter().filter(|c| c.label() == label).count();
    println!(
        "\n{} fixed, {} regressed, {} new, {} same\n",
        count("fixed"),
        count("REGRESSED"),
        count("new"),
        count("same")
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(passed: bool, reading: f64) -> Report {
        Report {
            models: vec![ModelReport {
                name: "figure".to_owned(),
                faces: 10,
                skipped: None,
                poses: Vec::new(),
                goals: vec![GoalRow {
                    goal: "fewest-supports".to_owned(),
                    passed,
                    broken: Vec::new(),
                    reading,
                    best_pose: None,
                    best_reading: None,
                    seconds: 0.1,
                }],
            }],
            ..Report::new("printer", "medium", 0)
        }
    }

    #[test]
    fn a_goal_that_passed_and_now_fails_has_regressed() {
        let changes = compare(&report(true, 40.0), &report(false, 90.0));
        assert_eq!(changes.len(), 1);
        assert!(changes[0].regressed());
        assert_eq!(changes[0].reading, (Some(40.0), 90.0));
    }

    #[test]
    fn a_pose_kept_as_a_row_comes_back_as_it_was_measured() {
        let pose = Pose {
            name: "goal: Least peel force".to_owned(),
            source: Source::Goal(OrientGoal::LeastPeel),
            rotation: Quat::from_rotation_x(0.5),
            on_plate: true,
            printout: Printout {
                contacts: 3,
                hanging: 1,
                top_contacts: 2,
                layers: 400,
                pull_n: 2.5,
                growth_mm2: 30.0,
                islands: 4,
                cups: 1,
                peels: 0,
                resin_ml: 5.0,
            },
            tilt_deg: 28.6,
            fits: false,
            seconds: 1.2,
        };
        let back = PoseRow::of(&pose)
            .pose()
            .expect("a row this run wrote reads back");
        assert_eq!(back.source, pose.source);
        assert_eq!(back.printout, pose.printout);
        assert_eq!(back.rotation, pose.rotation);
        assert_eq!((back.on_plate, back.fits), (true, false));
    }

    #[test]
    fn a_goal_the_baseline_never_had_is_new_and_not_a_regression() {
        let mut empty = report(true, 0.0);
        empty.models.clear();
        let changes = compare(&empty, &report(false, 1.0));
        assert_eq!(changes[0].was, None);
        assert!(!changes[0].regressed());
    }
}
