//! What a prepared model is scored on, read from `estimate --json`, and how one preparation
//! stands against the autopilot's: the rule is in `docs/cli.md`, `agent-lab`.

use std::cmp::Ordering;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// How much more resin or print time than the autopilot's still matches it.
pub const MARGIN: f64 = 0.10;

/// The readings of one check, as the command line reports them.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Score {
    pub status: Status,
    pub islands: u64,
    pub suction_cups: u64,
    pub trapped_pockets: u64,
    /// Contacts the supports found no room to stand under.
    pub no_room_for: u64,
    pub resin_ml: f64,
    pub print_time_s: u64,
}

/// The report's `status`, best first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Ok,
    Unclean,
    Failed,
}

/// Where a preparation stands against the autopilot's on the same model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Standing {
    /// Matches it, and is better on at least one reading.
    Beats,
    Matches,
    Behind,
}

/// A standing and the readings that decided it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Judged {
    pub standing: Standing,
    /// The readings worse than the autopilot's.
    pub worse: Vec<String>,
    /// The readings better than the autopilot's.
    pub better: Vec<String>,
}

impl Standing {
    pub fn holds(self) -> bool {
        self != Self::Behind
    }
}

impl Score {
    /// The score of an `estimate --json` report; one that says it failed has none.
    pub fn of(report: &Value) -> Result<Self> {
        if let Some(error) = report.get("error") {
            bail!("the check failed: {}", error["message"]);
        }
        let status = serde_json::from_value(report["status"].clone())
            .with_context(|| format!("the report's status {} is unknown", report["status"]))?;
        let print = report
            .get("print")
            .context("the report has no print: was a printer named?")?;
        let count = |value: &Value, what: &str| {
            value
                .as_u64()
                .with_context(|| format!("the report gives no {what}"))
        };
        let no_room_for = report["models"].as_array().map_or(0, |models| {
            models
                .iter()
                .filter_map(|model| model["supports"]["no_room_for"].as_u64())
                .sum()
        });
        Ok(Self {
            status,
            islands: count(&print["cured"]["islands"], "islands")?,
            suction_cups: count(&print["cured"]["suction_cups"], "suction cups")?,
            trapped_pockets: count(&report["slicing"]["trapped_pockets"], "trapped pockets")?,
            no_room_for,
            resin_ml: print["resin_mm3"]
                .as_f64()
                .context("the report gives no resin")?
                / 1000.0,
            print_time_s: count(&print["print_time_s"], "print time")?,
        })
    }
}

/// `prepared` against the autopilot's `baseline`: it matches when no status or count is worse
/// and resin and time are within `MARGIN` above.
pub fn judge(prepared: &Score, baseline: &Score) -> Judged {
    let mut worse = Vec::new();
    let mut better = Vec::new();
    let statuses = format!("status {:?} against {:?}", prepared.status, baseline.status);
    match prepared.status.cmp(&baseline.status) {
        Ordering::Greater => worse.push(statuses),
        Ordering::Less => better.push(statuses),
        Ordering::Equal => {}
    }
    let mut compare = |name: &str, mine: f64, theirs: f64, margin: f64| {
        if mine > theirs * (1.0 + margin) {
            worse.push(format!("{name} {mine} against {theirs}"));
        } else if mine < theirs * (1.0 - margin) {
            better.push(format!("{name} {mine} against {theirs}"));
        }
    };
    for (name, mine, theirs) in [
        ("islands", prepared.islands, baseline.islands),
        ("suction cups", prepared.suction_cups, baseline.suction_cups),
        (
            "trapped pockets",
            prepared.trapped_pockets,
            baseline.trapped_pockets,
        ),
        (
            "contacts with no room",
            prepared.no_room_for,
            baseline.no_room_for,
        ),
    ] {
        compare(name, mine as f64, theirs as f64, 0.0);
    }
    compare(
        "resin ml",
        round(prepared.resin_ml),
        round(baseline.resin_ml),
        MARGIN,
    );
    compare(
        "print s",
        prepared.print_time_s as f64,
        baseline.print_time_s as f64,
        MARGIN,
    );
    let standing = if !worse.is_empty() {
        Standing::Behind
    } else if better.is_empty() {
        Standing::Matches
    } else {
        Standing::Beats
    };
    Judged {
        standing,
        worse,
        better,
    }
}

fn round(ml: f64) -> f64 {
    (ml * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn clean() -> Score {
        Score {
            status: Status::Ok,
            islands: 0,
            suction_cups: 0,
            trapped_pockets: 0,
            no_room_for: 0,
            resin_ml: 10.0,
            print_time_s: 3600,
        }
    }

    #[test]
    fn a_report_is_scored_on_its_status_risks_resin_and_time() {
        let report = json!({
            "schema": 1,
            "status": "unclean",
            "models": [{ "supports": { "no_room_for": 2 } }, {}],
            "slicing": { "trapped_pockets": 1 },
            "print": {
                "print_time_s": 5400,
                "resin_mm3": 12500.0,
                "cured": { "islands": 3, "suction_cups": 1 },
            },
        });
        let score = Score::of(&report).expect("every reading is there");
        assert_eq!(score.status, Status::Unclean);
        assert_eq!((score.islands, score.suction_cups), (3, 1));
        assert_eq!((score.trapped_pockets, score.no_room_for), (1, 2));
        assert!(
            (score.resin_ml - 12.5).abs() < 1e-9,
            "12 500 mm³ is 12.5 ml, read {}",
            score.resin_ml
        );
        assert_eq!(score.print_time_s, 5400);
    }

    #[test]
    fn a_failed_run_has_no_score() {
        let report = json!({ "schema": 1, "error": { "message": "no such file" } });
        assert!(Score::of(&report).is_err());
    }

    #[test]
    fn the_same_preparation_matches_the_autopilot() {
        let judged = judge(&clean(), &clean());
        assert_eq!(judged.standing, Standing::Matches);
    }

    #[test]
    fn one_island_more_is_behind_whatever_it_saves() {
        let prepared = Score {
            islands: 1,
            resin_ml: 5.0,
            ..clean()
        };
        let judged = judge(&prepared, &clean());
        assert_eq!(judged.standing, Standing::Behind);
        assert_eq!(judged.worse, ["islands 1 against 0"]);
    }

    #[test]
    fn resin_within_the_margin_matches_and_past_it_is_behind() {
        let near = Score {
            resin_ml: 10.9,
            ..clean()
        };
        assert_eq!(judge(&near, &clean()).standing, Standing::Matches);
        let far = Score {
            resin_ml: 11.2,
            ..clean()
        };
        assert_eq!(judge(&far, &clean()).standing, Standing::Behind);
    }

    #[test]
    fn a_clean_print_where_the_autopilot_left_resin_trapped_beats_it() {
        let trapped = Score {
            status: Status::Unclean,
            trapped_pockets: 1,
            ..clean()
        };
        let judged = judge(&clean(), &trapped);
        assert_eq!(judged.standing, Standing::Beats);
        assert_eq!(
            judged.better.len(),
            2,
            "status and pockets: {:?}",
            judged.better
        );
    }
}
