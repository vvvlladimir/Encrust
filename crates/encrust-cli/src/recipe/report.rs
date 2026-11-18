//! What each step of a recipe decided for one model and what it read to decide, for `--json`
//! and the terminal.

use std::collections::BTreeMap;
use std::fmt;

use core_recipe::{Decision, Reading, Resolved, StepKind, Value};
use serde::Serialize;
use serde_json::Value as Json;

use super::settings::Settings;

/// A recipe as it went for one model.
#[derive(Debug, Clone, Serialize)]
pub struct RecipeRun {
    pub name: String,
    pub steps: Vec<StepRun>,
}

/// One step for one model: what was decided, on what, with which settings, and what it did.
#[derive(Debug, Clone, Serialize)]
pub struct StepRun {
    /// Its place in the recipe, from 0.
    pub index: usize,
    pub kind: &'static str,
    pub decision: Decided,
    pub read: BTreeMap<String, Json>,
    /// What it ran with, a flag given over the recipe's own; empty unless it ran.
    pub settings: BTreeMap<String, Json>,
    /// Why a step that ran did nothing, why it failed and was skipped, or what a gate or a
    /// pause said.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// The holes a drain step drilled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub holes: Option<usize>,
    /// How many times a check supported its islands and measured again.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retries: Option<u8>,
    /// Where a save step wrote the plate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saved: Option<String>,
}

/// What `core_recipe::decide` answered, as the report names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Decided {
    Run,
    Skip,
    Stop,
    Pause,
    Disabled,
}

impl StepRun {
    /// Step `index` of `kind` as `decision` answered it, run with `settings` when it runs.
    pub(super) fn of(
        index: usize,
        kind: StepKind,
        decision: &Decision,
        settings: &Settings,
    ) -> Self {
        let (decided, read, note): (Decided, &[Reading], Option<&str>) = match decision {
            Decision::Run { read, .. } => (Decided::Run, read, None),
            Decision::Skip { read } => (Decided::Skip, read, None),
            Decision::Stop { message, read } => (Decided::Stop, read, Some(message)),
            Decision::Pause { message } => (Decided::Pause, &[], Some(message)),
            Decision::Disabled => (Decided::Disabled, &[], None),
        };
        Self {
            index,
            kind: kind.name(),
            decision: decided,
            read: read
                .iter()
                .map(|reading| (reading.name.clone(), value_of(reading.value)))
                .collect(),
            settings: settings
                .iter()
                .map(|(key, value)| (key.clone(), resolved_of(value)))
                .collect(),
            note: note.filter(|note| !note.is_empty()).map(str::to_owned),
            holes: None,
            retries: None,
            saved: None,
        }
    }
}

fn value_of(value: Value) -> Json {
    match value {
        Value::Number(number) => Json::from(f64::from(number)),
        Value::Bool(flag) => Json::Bool(flag),
    }
}

fn resolved_of(value: &Resolved) -> Json {
    match value {
        Resolved::Number(number) => Json::from(f64::from(*number)),
        Resolved::Bool(flag) => Json::Bool(*flag),
        Resolved::Word(text) | Resolved::Text(text) => Json::String(text.clone()),
        Resolved::Points(points) => points
            .iter()
            .map(|point| point.iter().map(|at| f64::from(*at)).collect())
            .collect::<Vec<Vec<f64>>>()
            .into(),
    }
}

/// One line for the terminal: `  step 2  hollow    run   saves_ml 42, big_ml 30`.
impl fmt::Display for StepRun {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let decided = match self.decision {
            Decided::Run => "run",
            Decided::Skip => "skip",
            Decided::Stop => "stop",
            Decided::Pause => "pause",
            Decided::Disabled => "off",
        };
        write!(f, "  step {:<2} {:<9} {decided:<5}", self.index, self.kind)?;
        let read: Vec<String> = self
            .read
            .iter()
            .map(|(name, value)| format!("{name} {value}"))
            .collect();
        write!(f, "{}", read.join(", "))?;
        if let Some(note) = &self.note {
            write!(f, "  {note}")?;
        }
        Ok(())
    }
}
