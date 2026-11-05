//! The answer for one step of a plan on one model, and what it read to answer.

use std::collections::BTreeMap;

use core_geometry::Scalar;

use crate::condition::{Condition, Value};
use crate::error::RecipeError;
use crate::metrics::{Metric, Metrics};
use crate::recipe::{Setting, Settings, Step, StepKind};
use crate::validate::Plan;

#[cfg(test)]
mod tests;

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
    Text(String),
    /// `[x, y, z]` fractions of the placed box.
    Points(Vec<[Scalar; 3]>),
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
    decided(plan, index, metrics, false)
}

/// [`decide`] as though the step's `when` held: a step a front end runs whatever its
/// condition, as a typed flag asks (`docs/design/recipes.md`).
pub fn decide_forced(
    plan: &Plan,
    index: usize,
    metrics: &Metrics,
) -> Result<Decision, RecipeError> {
    decided(plan, index, metrics, true)
}

fn decided(
    plan: &Plan,
    index: usize,
    metrics: &Metrics,
    forced: bool,
) -> Result<Decision, RecipeError> {
    let step = step_at(plan, index)?;
    if !step.enabled {
        return Ok(Decision::Disabled);
    }
    let mut reader = Reader::new(plan, metrics);
    if let Some(when) = step.when.as_ref().filter(|_| !forced)
        && !reader.holds(when)?
    {
        return Ok(Decision::Skip { read: reader.read });
    }
    let message = step.message.clone().unwrap_or_default();
    match step.kind {
        StepKind::Gate => {
            let stops = match &step.stop_if {
                Some(stop_if) => reader.holds(stop_if)?,
                None => false,
            };
            Ok(if stops {
                Decision::Stop {
                    message,
                    read: reader.read,
                }
            } else {
                Decision::Skip { read: reader.read }
            })
        }
        StepKind::Pause => Ok(Decision::Pause { message }),
        _ => {
            let settings = reader.settings(step)?;
            Ok(Decision::Run {
                settings,
                read: reader.read,
            })
        }
    }
}

/// Whether `check` at `index` supports islands again after `attempt` tries.
pub fn retry(
    plan: &Plan,
    index: usize,
    metrics: &Metrics,
    attempt: u8,
) -> Result<bool, RecipeError> {
    let Some(retry) = &step_at(plan, index)?.retry else {
        return Ok(false);
    };
    if attempt >= retry.max {
        return Ok(false);
    }
    Reader::new(plan, metrics).holds(&retry.while_)
}

/// Every step as it can be decided before the run, on the model as `metrics` measured it
/// then; a step that reads a name not measured yet is decided when reached.
pub fn preview(plan: &Plan, metrics: &Metrics) -> Vec<Foreseen> {
    (0..plan.recipe().steps.len())
        .map(|index| match decide(plan, index, metrics) {
            Ok(decision) => Foreseen::Decided(decision),
            Err(_) => Foreseen::WhenReached,
        })
        .collect()
}

fn step_at(plan: &Plan, index: usize) -> Result<&Step, RecipeError> {
    plan.recipe()
        .steps
        .get(index)
        .ok_or(RecipeError::NoSuchStep { step: index })
}

/// Looks names up in a model's metrics, then the recipe's vars, noting each once.
struct Reader<'a> {
    plan: &'a Plan,
    metrics: &'a Metrics,
    read: Vec<Reading>,
}

impl<'a> Reader<'a> {
    fn new(plan: &'a Plan, metrics: &'a Metrics) -> Self {
        Self {
            plan,
            metrics,
            read: Vec::new(),
        }
    }

    fn value_of(&mut self, name: &str) -> Result<Value, RecipeError> {
        let value = match Metric::from_name(name) {
            Some(metric) => self
                .metrics
                .get(metric)
                .ok_or_else(|| RecipeError::Unmeasured { name: name.into() })?,
            None => self
                .plan
                .recipe()
                .vars
                .get(name)
                .map(|number| Value::Number(*number))
                .ok_or_else(|| RecipeError::UnknownName { name: name.into() })?,
        };
        if !self.read.iter().any(|reading| reading.name == name) {
            self.read.push(Reading {
                name: name.to_owned(),
                value,
            });
        }
        Ok(value)
    }

    fn eval(&mut self, condition: &Condition) -> Result<Value, RecipeError> {
        condition.eval(&mut |name| self.value_of(name))
    }

    fn holds(&mut self, condition: &Condition) -> Result<bool, RecipeError> {
        match self.eval(condition)? {
            Value::Bool(holds) => Ok(holds),
            Value::Number(_) => Err(RecipeError::WrongKind {
                text: condition.text().to_owned(),
                expected: crate::condition::Kind::Bool,
                found: crate::condition::Kind::Number,
            }),
        }
    }

    /// `set`, with the first `choose` that holds merged over it, every expression worked out.
    fn settings(&mut self, step: &Step) -> Result<BTreeMap<String, Resolved>, RecipeError> {
        let mut merged: Settings = step.set.clone();
        for choice in &step.choose {
            if self.holds(&choice.when)? {
                merged.extend(choice.set.clone());
                break;
            }
        }
        let mut resolved = BTreeMap::new();
        for (key, setting) in merged {
            let value = match setting {
                Setting::Number(number) => Resolved::Number(number),
                Setting::Bool(flag) => Resolved::Bool(flag),
                Setting::Word(word) => Resolved::Word(word),
                Setting::Text(text) => Resolved::Text(text),
                Setting::Points(points) => Resolved::Points(points),
                Setting::Expr(condition) => match self.eval(&condition)? {
                    Value::Number(number) => Resolved::Number(number),
                    Value::Bool(flag) => Resolved::Bool(flag),
                },
            };
            resolved.insert(key, value);
        }
        Ok(resolved)
    }
}
