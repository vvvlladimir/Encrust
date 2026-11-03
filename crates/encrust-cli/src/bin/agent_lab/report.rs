//! A run as rows — the autopilot and each agent on each model — printed, kept as JSON, and
//! set beside a run kept before.

use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::score::{Judged, Score, Standing};

/// A whole run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub printer: String,
    pub resin: String,
    pub models: Vec<ModelRow>,
}

/// One model: the autopilot's preparation and each agent's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelRow {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skipped: Option<String>,
    pub autopilot: Prepared,
    #[serde(default)]
    pub agents: Vec<AgentRow>,
}

/// What one preparation scored, or why it has no score.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Prepared {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<Score>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why_not: Option<String>,
    pub seconds: f64,
}

/// One agent on one model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentRow {
    pub agent: String,
    pub prepared: Prepared,
    pub tool_calls: usize,
    pub failed_calls: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<u64>,
    /// Against the autopilot, when both have a score.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub judged: Option<Judged>,
}

/// Something that changed since the run kept before.
#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    /// An agent that matched or beat the autopilot on the model no longer does.
    Fell { model: String, agent: String },
    /// An agent now matches or beats the autopilot where it did not.
    Rose { model: String, agent: String },
    /// The autopilot's own score is worse than it was.
    AutopilotWorse { model: String, worse: Vec<String> },
}

impl Change {
    pub fn regressed(&self) -> bool {
        !matches!(self, Self::Rose { .. })
    }
}

impl AgentRow {
    fn holds(&self) -> bool {
        self.judged
            .as_ref()
            .is_some_and(|judged| judged.standing.holds())
    }
}

impl Report {
    pub fn write(&self, path: &Path) -> Result<()> {
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(path, text).with_context(|| format!("cannot write {}", path.display()))
    }

    pub fn read(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read {}", path.display()))?;
        serde_json::from_str(&text).with_context(|| format!("{} is not a run", path.display()))
    }

    /// Of every agent row with a score, how many match or beat the autopilot.
    pub fn tally(&self) -> (usize, usize) {
        let judged: Vec<&AgentRow> = self
            .models
            .iter()
            .flat_map(|model| &model.agents)
            .filter(|row| row.judged.is_some())
            .collect();
        (
            judged.iter().filter(|row| row.holds()).count(),
            judged.len(),
        )
    }
}

/// What changed from `kept` to `now`, model by model and agent by agent; a model or agent
/// in only one of them is not compared.
pub fn compare(kept: &Report, now: &Report) -> Vec<Change> {
    let mut changes = Vec::new();
    for model in &now.models {
        let Some(before) = kept.models.iter().find(|old| old.name == model.name) else {
            continue;
        };
        if let (Some(old), Some(new)) = (&before.autopilot.score, &model.autopilot.score) {
            let judged = crate::score::judge(new, old);
            if judged.standing == Standing::Behind {
                changes.push(Change::AutopilotWorse {
                    model: model.name.clone(),
                    worse: judged.worse,
                });
            }
        }
        for row in &model.agents {
            let Some(old) = before.agents.iter().find(|old| old.agent == row.agent) else {
                continue;
            };
            let named = || (model.name.clone(), row.agent.clone());
            match (old.holds(), row.holds()) {
                (true, false) => {
                    let (model, agent) = named();
                    changes.push(Change::Fell { model, agent });
                }
                (false, true) => {
                    let (model, agent) = named();
                    changes.push(Change::Rose { model, agent });
                }
                _ => {}
            }
        }
    }
    changes
}

pub fn print_model(model: &ModelRow) {
    println!("## {}", model.name);
    if let Some(why) = &model.skipped {
        println!("skipped: {why}\n");
        return;
    }
    println!("{:<16} {}", "autopilot", line(&model.autopilot));
    for row in &model.agents {
        let tokens = row.tokens.map_or_else(
            || "? tokens".to_owned(),
            |tokens| format!("{tokens} tokens"),
        );
        println!(
            "{:<16} {} · {} calls ({} failed), {tokens}",
            row.agent,
            line(&row.prepared),
            row.tool_calls,
            row.failed_calls
        );
        if let Some(judged) = &row.judged {
            let mut said = format!("{:<16} {:?}", "", judged.standing);
            for (sign, readings) in [("-", &judged.worse), ("+", &judged.better)] {
                for reading in readings {
                    said.push_str(&format!("; {sign} {reading}"));
                }
            }
            println!("{said}");
        }
    }
    println!();
}

fn line(prepared: &Prepared) -> String {
    match (&prepared.score, &prepared.why_not) {
        (Some(score), _) => format!(
            "{:?}: {} islands, {} cups, {} trapped, {} no room, {:.2} ml, {} s print · {:.0} s",
            score.status,
            score.islands,
            score.suction_cups,
            score.trapped_pockets,
            score.no_room_for,
            score.resin_ml,
            score.print_time_s,
            prepared.seconds
        ),
        (None, why) => format!(
            "no score: {} · {:.0} s",
            why.as_deref().unwrap_or("none given"),
            prepared.seconds
        ),
    }
}

pub fn print_changes(changes: &[Change]) {
    if changes.is_empty() {
        println!("\nNothing changed against the baseline.");
        return;
    }
    println!("\nAgainst the baseline:");
    for change in changes {
        match change {
            Change::Fell { model, agent } => {
                println!("- {model}: {agent} no longer matches the autopilot");
            }
            Change::Rose { model, agent } => {
                println!("+ {model}: {agent} now matches or beats the autopilot");
            }
            Change::AutopilotWorse { model, worse } => {
                println!("- {model}: the autopilot is worse: {}", worse.join("; "));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::score::Status;

    fn score(islands: u64) -> Score {
        Score {
            status: Status::Ok,
            islands,
            suction_cups: 0,
            trapped_pockets: 0,
            no_room_for: 0,
            resin_ml: 10.0,
            print_time_s: 3600,
        }
    }

    fn prepared(islands: u64) -> Prepared {
        Prepared {
            score: Some(score(islands)),
            why_not: None,
            seconds: 1.0,
        }
    }

    fn run(autopilot_islands: u64, standing: Standing) -> Report {
        Report {
            printer: "p".into(),
            resin: "r".into(),
            models: vec![ModelRow {
                name: "cube".into(),
                skipped: None,
                autopilot: prepared(autopilot_islands),
                agents: vec![AgentRow {
                    agent: "sonnet".into(),
                    prepared: prepared(0),
                    tool_calls: 4,
                    failed_calls: 0,
                    tokens: Some(1000),
                    judged: Some(Judged {
                        standing,
                        worse: Vec::new(),
                        better: Vec::new(),
                    }),
                }],
            }],
        }
    }

    #[test]
    fn an_agent_that_falls_behind_the_autopilot_is_a_regression() {
        let changes = compare(&run(0, Standing::Matches), &run(0, Standing::Behind));
        assert_eq!(
            changes,
            [Change::Fell {
                model: "cube".into(),
                agent: "sonnet".into()
            }]
        );
        assert!(changes[0].regressed());
    }

    #[test]
    fn an_agent_that_catches_up_is_no_regression() {
        let changes = compare(&run(0, Standing::Behind), &run(0, Standing::Beats));
        assert!(!changes.iter().any(Change::regressed), "{changes:?}");
    }

    #[test]
    fn an_autopilot_that_leaves_an_island_it_did_not_is_a_regression() {
        let changes = compare(&run(0, Standing::Matches), &run(1, Standing::Matches));
        assert!(
            matches!(&changes[..], [Change::AutopilotWorse { model, .. }] if model == "cube"),
            "{changes:?}"
        );
    }

    #[test]
    fn a_run_reads_back_as_it_was_kept() {
        let path = std::env::temp_dir().join(format!("agent-lab-run-{}.json", std::process::id()));
        let kept = run(0, Standing::Beats);
        kept.write(&path).expect("the temp dir is writable");
        assert_eq!(Report::read(&path).expect("it was just written"), kept);
        assert_eq!(kept.tally(), (1, 1));
    }
}
