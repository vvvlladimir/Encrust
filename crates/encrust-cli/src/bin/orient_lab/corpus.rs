//! The models a run benches and what a person expects of each, read from a TOML file so a
//! corpus kept outside the repository carries its own expectations.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use core_plate::OrientGoal;
use serde::Deserialize;

use crate::verdict::Expect;

/// One model of a corpus.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// Resolved against the corpus file's own directory.
    pub path: PathBuf,
    pub note: String,
    /// Why the model is listed but not benched, such as a format nothing reads yet.
    pub skip: Option<String>,
    pub expect: Expectations,
    pub isolated: Isolated,
}

/// Goals a person has taken out of the count for one model, and why: benched and shown,
/// but neither a pass nor a fail, so a model no pose can satisfy does not hide progress.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Isolated {
    /// Why every goal of the model is isolated.
    pub all: Option<String>,
    /// Why one goal is, by its slug.
    pub goals: BTreeMap<String, String>,
}

impl Isolated {
    /// Why `goal` is isolated, or `None` when it counts.
    pub fn why(&self, goal_slug: &str) -> Option<&str> {
        self.goals
            .get(goal_slug)
            .or(self.all.as_ref())
            .map(String::as_str)
    }
}

/// What a person expects of a model under every goal, then under one goal in particular.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Expectations {
    pub max_tilt_deg: Option<f32>,
    pub on_plate: Option<bool>,
    pub fewest_supports: Option<Expect>,
    pub least_peel: Option<Expect>,
    pub shortest_print: Option<Expect>,
    pub clean_top: Option<Expect>,
}

impl Expectations {
    /// What `goal` is held to: its own table where it sets a field, every goal's otherwise.
    pub fn for_goal(&self, goal: OrientGoal) -> Expect {
        let own = match goal {
            OrientGoal::FewestSupports => self.fewest_supports,
            OrientGoal::LeastPeel => self.least_peel,
            OrientGoal::ShortestPrint => self.shortest_print,
            OrientGoal::CleanTop => self.clean_top,
        }
        .unwrap_or_default();
        Expect {
            max_tilt_deg: own.max_tilt_deg.or(self.max_tilt_deg),
            on_plate: own.on_plate.or(self.on_plate),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    model: Vec<Raw>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    path: PathBuf,
    #[serde(default)]
    note: String,
    skip: Option<String>,
    #[serde(default)]
    expect: Expectations,
    isolate: Option<String>,
    #[serde(default, rename = "isolate-goals")]
    isolate_goals: BTreeMap<String, String>,
}

/// Reads the corpus at `path`.
pub fn read(path: &Path) -> Result<Vec<Entry>> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    let base = path.parent().unwrap_or(Path::new("."));
    parse(&text, base).with_context(|| format!("cannot read the corpus {}", path.display()))
}

fn parse(text: &str, base: &Path) -> Result<Vec<Entry>> {
    let file: File = toml::from_str(text)?;
    Ok(file
        .model
        .into_iter()
        .map(|raw| Entry {
            path: base.join(raw.path),
            note: raw.note,
            skip: raw.skip,
            expect: raw.expect,
            isolated: Isolated {
                all: raw.isolate,
                goals: raw.isolate_goals,
            },
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_goals_own_expectation_wins_over_every_goals() {
        let entries = parse(
            r#"
            [[model]]
            path = "models/figure.stl"
            note = "a figure"

            [model.expect]
            max-tilt-deg = 90

            [model.expect.shortest-print]
            max-tilt-deg = 180
            on-plate = false
            "#,
            Path::new("/corpus"),
        )
        .expect("the corpus is well formed");

        let entry = &entries[0];
        assert_eq!(entry.path, Path::new("/corpus/models/figure.stl"));
        let fewest = entry.expect.for_goal(OrientGoal::FewestSupports);
        assert_eq!(fewest.max_tilt_deg, Some(90.0));
        assert_eq!(fewest.on_plate, None);
        let shortest = entry.expect.for_goal(OrientGoal::ShortestPrint);
        assert_eq!(shortest.max_tilt_deg, Some(180.0));
        assert_eq!(shortest.on_plate, Some(false));
    }

    #[test]
    fn a_goal_isolated_on_its_own_says_its_own_reason() {
        let entries = parse(
            r#"
            [[model]]
            path = "mask.3mf"
            isolate-goals = { least-peel = "every pose cups" }
            "#,
            Path::new("."),
        )
        .expect("the corpus is well formed");
        let isolated = &entries[0].isolated;
        assert_eq!(isolated.why("least-peel"), Some("every pose cups"));
        assert_eq!(isolated.why("fewest-supports"), None);
    }

    #[test]
    fn a_misspelt_expectation_is_refused() {
        let parsed = parse(
            "[[model]]\npath = \"a.stl\"\n[model.expect]\nmax-tilt = 10\n",
            Path::new("."),
        );
        assert!(parsed.is_err(), "a typo must not silently expect nothing");
    }
}
