//! What a run found, as a table to read and as JSON to keep, and how it compares with a
//! run kept before.

use std::path::Path;

use anyhow::{Context, Result};
use core_engine::Printout;
use core_geometry::Quat;
use core_plate::OrientGoal;
use serde::{Deserialize, Serialize};

use crate::poses::{Pose, Source, Trace};
use crate::verdict::Verdict;

/// The goal poses `model` held in this report, to be measured again in a run compared with
/// it.
pub fn known(report: &Report, model: &str) -> Vec<crate::poses::Known> {
    report
        .models
        .iter()
        .filter(|m| m.name == model)
        .flat_map(|m| &m.poses)
        .filter(|pose| pose.source != "hand" && pose.source != "oracle")
        .map(|pose| {
            (
                pose.name.trim_start_matches("known: ").to_owned(),
                Quat::from_array(pose.rotation),
                pose.on_plate,
            )
        })
        .collect()
}

/// Bumped when a field changes meaning, so an old baseline is refused rather than misread.
const SCHEMA: u32 = 2;

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
    /// Whether it has a footprint flat on the plate; true in runs kept before it was read.
    #[serde(default = "sits_by_default")]
    pub sits: bool,
    pub contacts: usize,
    /// The resin in the supports, millilitres; zero in runs kept before it was read.
    #[serde(default)]
    pub supports_ml: f32,
    pub hanging: usize,
    pub top_contacts: usize,
    pub layers: usize,
    /// How tall the stack stands, millimetres; zero in runs kept before it was read.
    #[serde(default)]
    pub stack_mm: f32,
    pub pull_n: f64,
    pub growth_mm2: f32,
    pub islands: usize,
    pub cups: usize,
    pub peels: usize,
    pub resin_ml: f32,
    pub seconds: f64,
    /// How a goal made its choice; empty for any other pose and in runs kept before it.
    #[serde(default)]
    pub trace: Trace,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalRow {
    pub goal: String,
    /// `pass`, `fail`, `unreachable` — no pose measured would pass — or `isolated` by a
    /// person; only the first two count.
    pub status: String,
    /// Why a person isolated it.
    pub isolated: Option<String>,
    /// A measured pose that would pass, or `None` when none would.
    pub reachable_by: Option<String>,
    /// The rules the choice broke, by name.
    pub broken: Vec<String>,
    pub reading: f64,
    pub best_pose: Option<String>,
    pub best_reading: Option<f64>,
    pub seconds: f64,
}

fn sits_by_default() -> bool {
    true
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
pub fn goal_of(slug_text: &str) -> Option<OrientGoal> {
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
            "known" => Source::Known,
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
                supports_ml: self.supports_ml,
                hanging: self.hanging,
                top_contacts: self.top_contacts,
                layers: self.layers,
                stack_mm: self.stack_mm,
                pull_n: self.pull_n,
                growth_mm2: self.growth_mm2,
                islands: self.islands,
                cups: self.cups,
                peels: self.peels,
                resin_ml: self.resin_ml,
                overflows: !self.fits,
            },
            tilt_deg: self.tilt_deg,
            fits: self.fits,
            sits: self.sits,
            seconds: self.seconds,
            trace: self.trace.clone(),
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
                Source::Known => "known".to_owned(),
            },
            rotation: pose.rotation.to_array(),
            tilt_deg: pose.tilt_deg,
            on_plate: pose.on_plate,
            fits: pose.fits,
            sits: pose.sits,
            contacts: o.contacts,
            supports_ml: o.supports_ml,
            hanging: o.hanging,
            top_contacts: o.top_contacts,
            layers: o.layers,
            stack_mm: o.stack_mm,
            pull_n: o.pull_n,
            growth_mm2: o.growth_mm2,
            islands: o.islands,
            cups: o.cups,
            peels: o.peels,
            resin_ml: o.resin_ml,
            seconds: pose.seconds,
            trace: pose.trace.clone(),
        }
    }
}

impl GoalRow {
    pub fn of(verdict: &Verdict, seconds: f64, isolated: Option<&str>) -> Self {
        let status = if isolated.is_some() {
            "isolated"
        } else {
            verdict.status().name()
        };
        Self {
            goal: slug(verdict.goal).to_owned(),
            status: status.to_owned(),
            isolated: isolated.map(str::to_owned),
            reachable_by: verdict.reachable_by.clone(),
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

    /// Goals passed and goals counted: those that pass or fail, not those out of reach
    /// or isolated.
    pub fn score(&self) -> (usize, usize) {
        let goals = self.models.iter().flat_map(|model| &model.goals);
        let counted = goals
            .clone()
            .filter(|goal| goal.status == "pass" || goal.status == "fail")
            .count();
        (goals.filter(|goal| goal.status == "pass").count(), counted)
    }

    /// Seconds every goal's choice spent searching and proving, summed over the run.
    pub fn stages(&self) -> (f64, f64) {
        self.models.iter().flat_map(|model| &model.poses).fold(
            (0.0, 0.0),
            |(search_s, proof_s), pose| {
                (search_s + pose.trace.search_s, proof_s + pose.trace.proof_s)
            },
        )
    }

    /// Model and goal of every goal no measured pose could pass, and not yet isolated.
    pub fn unreachable(&self) -> Vec<(&str, &str)> {
        self.models
            .iter()
            .flat_map(|model| {
                model
                    .goals
                    .iter()
                    .filter(|goal| goal.status == "unreachable")
                    .map(|goal| (model.name.as_str(), goal.goal.as_str()))
            })
            .collect()
    }
}

/// Prints one model's poses and verdicts as markdown.
pub fn print_model(model: &ModelReport, verdicts: &[Verdict]) {
    print_poses(model);
    for (verdict, row) in verdicts.iter().zip(&model.goals) {
        let call = match row.status.as_str() {
            "pass" => "pass".to_owned(),
            "fail" => format!(
                "FAIL (reachable: {})",
                row.reachable_by.as_deref().unwrap_or("—")
            ),
            "unreachable" => "UNREACHABLE: no measured pose passes".to_owned(),
            _ => format!("isolated: {}", row.isolated.as_deref().unwrap_or("")),
        };
        println!("- {}: **{call}**", verdict.goal.label());
        for check in verdict.checks.iter().filter(|check| !check.passed) {
            println!("  - {}: {}", check.rule.name(), check.detail);
        }
        let chosen = model.poses.iter().find(|pose| pose.source == row.goal);
        let full = |pose: &&PoseRow| pose.trace.finalists.iter().any(|f| f.full.is_some());
        if let Some(chosen) = chosen.filter(|pose| row.status != "pass" || full(pose)) {
            print_trace(&chosen.trace);
        }
    }
    println!();
}

/// The finalists a goal printed and why each ended where it did, read on the coarse stack
/// the proof ranks on, not the full one the table above holds.
fn print_trace(trace: &Trace) {
    println!(
        "  - trace: search {:.1} s, proof {:.1} s, finalists on the coarse stack:",
        trace.search_s, trace.proof_s
    );
    for finalist in &trace.finalists {
        let stand = if finalist.on_plate {
            "on the plate"
        } else {
            "on supports"
        };
        let full = finalist.full.map_or_else(String::new, |full| {
            format!(
                " (full: {} fatal, {} unheld, reads {:.2})",
                full.fatal, full.unheld, full.reading
            )
        });
        println!(
            "    - #{} {:.0}° {stand}: {} fatal, {} unheld, reads {:.2}{full} — {}",
            finalist.searched,
            finalist.tilt_deg,
            finalist.fatal,
            finalist.unheld,
            finalist.reading,
            finalist.ruling
        );
    }
}

fn print_poses(model: &ModelReport) {
    println!("## {} — {} faces\n", model.name, model.faces);
    println!(
        "| pose | tilt ° | plate | fits | contacts | supports ml | hanging | on top | layers | pull N | islands | cups | peels | resin ml | s |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|");
    for p in &model.poses {
        println!(
            "| {} | {:.0} | {} | {} | {} | {:.2} | {} | {} | {} | {:.1} | {} | {} | {} | {:.1} | {:.1} |",
            p.name,
            p.tilt_deg,
            if p.on_plate { "yes" } else { "—" },
            if p.fits { "yes" } else { "NO" },
            p.contacts,
            p.supports_ml,
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
}

/// How one goal of one model moved between two runs.
#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    pub model: String,
    pub goal: String,
    /// The status before, or `None` for a goal the baseline never had.
    pub was: Option<String>,
    pub now: String,
    pub reading: (Option<f64>, f64),
}

impl Change {
    /// A goal that passed before and does not now, unless a person isolated it since.
    pub fn regressed(&self) -> bool {
        self.was.as_deref() == Some("pass") && self.now != "pass" && self.now != "isolated"
    }

    fn label(&self) -> &'static str {
        match (self.was.as_deref(), self.now.as_str()) {
            (None, _) => "new",
            (Some(was), now) if was == now => "same",
            _ if self.regressed() => "REGRESSED",
            (_, "pass") => "fixed",
            (_, "isolated") => "isolated",
            (_, "unreachable") => "unreachable",
            _ => "changed",
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
                    was: old.map(|old| old.status.clone()),
                    now: goal.status.clone(),
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
        "\n{} fixed, {} regressed, {} unreachable, {} isolated, {} new, {} same\n",
        count("fixed"),
        count("REGRESSED"),
        count("unreachable"),
        count("isolated"),
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
                    status: if passed { "pass" } else { "fail" }.to_owned(),
                    isolated: None,
                    reachable_by: None,
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
                supports_ml: 0.4,
                hanging: 1,
                top_contacts: 2,
                layers: 400,
                stack_mm: 20.0,
                pull_n: 2.5,
                growth_mm2: 30.0,
                islands: 4,
                cups: 1,
                peels: 0,
                resin_ml: 5.0,
                overflows: true,
            },
            tilt_deg: 28.6,
            fits: false,
            sits: true,
            seconds: 1.2,
            trace: Trace::default(),
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

    #[test]
    fn a_goal_a_person_isolated_since_is_not_a_regression() {
        let mut isolated = report(false, 1.0);
        isolated.models[0].goals[0].status = "isolated".to_owned();
        let changes = compare(&report(true, 1.0), &isolated);
        assert!(!changes[0].regressed());
    }
}
