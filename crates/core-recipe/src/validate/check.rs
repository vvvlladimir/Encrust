//! The names, kinds, timing and bounds of a recipe's conditions and settings.

use std::collections::BTreeMap;

use core_geometry::Scalar;

use crate::condition::{Condition, Kind};
use crate::error::RecipeError;
use crate::metrics::Metric;
use crate::recipe::{MAX_RETRY, Recipe, Setting, SettingKind, Settings, StepKind};

pub(super) fn check(recipe: &Recipe) -> Result<(), RecipeError> {
    if let Some(name) = recipe
        .vars
        .keys()
        .find(|name| Metric::from_name(name).is_some())
    {
        return Err(RecipeError::Shadowed { name: name.clone() });
    }
    for (index, step) in recipe.steps.iter().enumerate() {
        let scope = Scope {
            vars: &recipe.vars,
            before: &recipe.steps[..index],
            step: index,
            kind: step.kind,
        };
        let conditions = step
            .when
            .iter()
            .chain(&step.stop_if)
            .chain(step.choose.iter().map(|choice| &choice.when));
        for condition in conditions {
            scope.expect(condition, Kind::Bool, false)?;
        }
        if let Some(retry) = &step.retry {
            scope.expect(&retry.while_, Kind::Bool, true)?;
            if retry.max > MAX_RETRY {
                return Err(RecipeError::RetryTooMany {
                    step: index,
                    max: retry.max,
                    limit: MAX_RETRY,
                });
            }
        }
        scope.settings(&step.set)?;
        for choice in &step.choose {
            scope.settings(&choice.set)?;
        }
    }
    Ok(())
}

struct Scope<'a> {
    vars: &'a BTreeMap<String, Scalar>,
    before: &'a [crate::recipe::Step],
    step: usize,
    kind: StepKind,
}

impl Scope<'_> {
    fn kind_of(&self, name: &str) -> Option<Kind> {
        match Metric::from_name(name) {
            Some(metric) => Some(metric.kind()),
            None => self.vars.contains_key(name).then_some(Kind::Number),
        }
    }

    fn measured(&self, after: &[StepKind], own_check: bool) -> bool {
        (own_check && self.kind == StepKind::Check && after.contains(&self.kind))
            || self
                .before
                .iter()
                .any(|step| step.enabled && after.contains(&step.kind))
    }

    fn expect(
        &self,
        condition: &Condition,
        expected: Kind,
        own_check: bool,
    ) -> Result<(), RecipeError> {
        let found = condition.kind(&|name| self.kind_of(name))?;
        if found != expected {
            return Err(wrong_kind(condition, expected, found));
        }
        for name in condition.names() {
            let after = Metric::from_name(name).map_or(&[][..], Metric::known_after);
            if !after.is_empty() && !self.measured(after, own_check) {
                return Err(RecipeError::NotYetKnown {
                    step: self.step,
                    name: name.to_owned(),
                    after: after[0],
                });
            }
        }
        Ok(())
    }

    fn settings(&self, settings: &Settings) -> Result<(), RecipeError> {
        for (key, setting) in settings {
            let Setting::Expr(condition) = setting else {
                continue;
            };
            let wanted = self.kind.settings().iter().find(|(name, _)| name == key);
            match wanted.map(|(_, kind)| kind) {
                Some(SettingKind::Number) => self.expect(condition, Kind::Number, false)?,
                Some(SettingKind::Bool) => self.expect(condition, Kind::Bool, false)?,
                _ => {
                    return Err(RecipeError::BadSetting {
                        step: self.step,
                        key: key.clone(),
                        expected: "a value, not an expression".into(),
                        found: format!("= {}", condition.text()),
                    });
                }
            }
        }
        Ok(())
    }
}

fn wrong_kind(condition: &Condition, expected: Kind, found: Kind) -> RecipeError {
    RecipeError::WrongKind {
        text: condition.text().to_owned(),
        expected,
        found,
    }
}
