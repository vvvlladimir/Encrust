//! The answer for one step of a plan on one model, and what it read to answer.

use std::collections::BTreeMap;

use core_geometry::Scalar;

use crate::condition::Value;
use crate::error::RecipeError;
use crate::metrics::Metrics;
use crate::validate::Plan;

/// A name a decision read, and its value then, for the plan shown and the report.
#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    pub name: String,
    pub value: Value,
}

/// A setting with its expression worked out.
#[derive(Debug, Clone, PartialEq)]
pub enum Resolved {
    Number(Scalar),
    Bool(bool),
    Word(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Decision {
    /// Run the step with these settings; a key left out takes the engine's default.
    Run {
        settings: BTreeMap<String, Resolved>,
        read: Vec<Reading>,
    },
    /// Its `when` does not hold.
    Skip {
        read: Vec<Reading>,
    },
    /// A gate whose `stop_if` holds: the run ends, the plate put back.
    Stop {
        message: String,
        read: Vec<Reading>,
    },
    Pause {
        message: String,
    },
    /// `enabled = false`.
    Disabled,
}

/// A step's decision before the run: known now, or only once the step is reached.
#[derive(Debug, Clone, PartialEq)]
pub enum Foreseen {
    Decided(Decision),
    WhenReached,
}

/// Decides step `index` of `plan` for a model measured as `metrics`.
pub fn decide(plan: &Plan, index: usize, metrics: &Metrics) -> Result<Decision, RecipeError> {
    // TODO(step-4): `when`, then `choose` over `set`, then each `=` setting; a gate's `stop_if`.
    let _ = (plan, index, metrics);
    todo!()
}

/// Whether `check` at `index` supports islands again after `attempt` tries.
pub fn retry(
    plan: &Plan,
    index: usize,
    metrics: &Metrics,
    attempt: u8,
) -> Result<bool, RecipeError> {
    // TODO(step-4): `while_` holds and `attempt` is under `max`.
    let _ = (plan, index, metrics, attempt);
    todo!()
}

/// Every step as it can be decided before the run, for the plan shown on the button.
pub fn preview(plan: &Plan, metrics: &Metrics) -> Vec<Foreseen> {
    // TODO(step-4): `decide`, with `Unmeasured` and every step after a work step whose
    // results a condition reads turned into `WhenReached`.
    let _ = (plan, metrics);
    todo!()
}
