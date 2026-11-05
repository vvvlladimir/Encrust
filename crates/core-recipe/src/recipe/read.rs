//! A recipe file's text into a `Recipe`: keys and kinds checked, order left to `validate`.

use std::collections::BTreeMap;

use core_geometry::Scalar;
use toml::{Table, Value};

use super::{
    Choice, FORMAT, OnFail, Recipe, Retry, Setting, SettingKind, Settings, Step, StepKind,
};
use crate::condition::Condition;
use crate::error::RecipeError;

const STEP_KEYS: [&str; 9] = [
    "do", "enabled", "when", "set", "choose", "on_fail", "message", "stop_if", "retry",
];

fn toml_error(message: impl Into<String>) -> RecipeError {
    RecipeError::Toml {
        message: message.into(),
    }
}

fn bad(step: usize, key: &str, expected: impl Into<String>, found: &Value) -> RecipeError {
    RecipeError::BadSetting {
        step,
        key: key.to_owned(),
        expected: expected.into(),
        found: found.to_string(),
    }
}

pub(super) fn recipe_from_toml(text: &str) -> Result<Recipe, RecipeError> {
    let table: Table = text
        .parse()
        .map_err(|e: toml::de::Error| toml_error(e.message()))?;
    if let Some(key) = table
        .keys()
        .find(|key| !["name", "format", "vars", "step"].contains(&key.as_str()))
    {
        return Err(toml_error(format!("`{key}` is not a key of a recipe")));
    }
    let found = match table.get("format") {
        None => 1,
        Some(Value::Integer(n)) => {
            u32::try_from(*n).map_err(|_| toml_error("`format` must be a positive whole number"))?
        }
        Some(_) => return Err(toml_error("`format` must be a whole number")),
    };
    if found > FORMAT {
        return Err(RecipeError::Format {
            found,
            supported: FORMAT,
        });
    }
    let name = match table.get("name") {
        Some(Value::String(name)) => name.clone(),
        _ => return Err(toml_error("`name` is required and must be text")),
    };
    let steps = match table.get("step") {
        None => Vec::new(),
        Some(Value::Array(items)) => items
            .iter()
            .enumerate()
            .map(|(index, item)| read_step(index, item))
            .collect::<Result<_, _>>()?,
        Some(_) => return Err(toml_error("`step` must be an array of tables, `[[step]]`")),
    };
    Ok(Recipe {
        name,
        vars: read_vars(table.get("vars"))?,
        steps,
    })
}

fn number(value: &Value) -> Option<Scalar> {
    match value {
        Value::Integer(n) => Some(*n as Scalar),
        Value::Float(f) => Some(*f as Scalar),
        _ => None,
    }
}

fn read_vars(vars: Option<&Value>) -> Result<BTreeMap<String, Scalar>, RecipeError> {
    let Some(vars) = vars else {
        return Ok(BTreeMap::new());
    };
    let Value::Table(vars) = vars else {
        return Err(toml_error("`vars` must be a table"));
    };
    vars.iter()
        .map(|(key, value)| {
            number(value)
                .map(|n| (key.clone(), n))
                .ok_or_else(|| toml_error(format!("var `{key}` must be a number")))
        })
        .collect()
}

fn condition(step: usize, key: &str, value: &Value) -> Result<Condition, RecipeError> {
    match value {
        Value::String(text) => Condition::parse(text),
        _ => Err(bad(step, key, "a condition in a string", value)),
    }
}

fn read_step(index: usize, item: &Value) -> Result<Step, RecipeError> {
    let Value::Table(table) = item else {
        return Err(toml_error(format!("step {index} must be a table")));
    };
    let kind = read_kind(index, table)?;
    refuse_unknown_keys(index, kind, table)?;
    let read_condition = |key: &str| {
        table
            .get(key)
            .map(|value| condition(index, key, value))
            .transpose()
    };
    let when = read_condition("when")?;
    let stop_if = read_condition("stop_if")?;
    if kind == StepKind::Gate && stop_if.is_none() {
        return Err(RecipeError::MissingSetting {
            step: index,
            kind,
            key: "stop_if",
        });
    }
    Ok(Step {
        kind,
        enabled: read_enabled(index, table.get("enabled"))?,
        when,
        set: read_settings(index, kind, table.get("set"))?,
        choose: read_choose(index, kind, table.get("choose"))?,
        on_fail: read_on_fail(index, table.get("on_fail"))?,
        message: read_message(index, table.get("message"))?,
        stop_if,
        retry: table
            .get("retry")
            .map(|value| read_retry(index, value))
            .transpose()?,
    })
}

/// The kind of step `index` its `do` names.
fn read_kind(index: usize, table: &Table) -> Result<StepKind, RecipeError> {
    match table.get("do") {
        Some(Value::String(name)) => {
            StepKind::from_name(name).ok_or_else(|| RecipeError::UnknownStep {
                step: index,
                name: name.clone(),
            })
        }
        Some(other) => Err(bad(index, "do", "a step's name", other)),
        None => Err(toml_error(format!("step {index} has no `do`"))),
    }
}

/// Refuses a key no step takes, or one a step of `kind` does not.
fn refuse_unknown_keys(index: usize, kind: StepKind, table: &Table) -> Result<(), RecipeError> {
    let refused = table.keys().find(|key| match key.as_str() {
        "message" => !matches!(kind, StepKind::Gate | StepKind::Pause),
        "stop_if" => kind != StepKind::Gate,
        "retry" => kind != StepKind::Check,
        other => !STEP_KEYS.contains(&other),
    });
    match refused {
        Some(key) => Err(RecipeError::UnknownSetting {
            step: index,
            kind,
            key: key.clone(),
        }),
        None => Ok(()),
    }
}

fn read_enabled(step: usize, value: Option<&Value>) -> Result<bool, RecipeError> {
    match value {
        None => Ok(true),
        Some(Value::Boolean(on)) => Ok(*on),
        Some(other) => Err(bad(step, "enabled", "true or false", other)),
    }
}

fn read_message(step: usize, value: Option<&Value>) -> Result<Option<String>, RecipeError> {
    match value {
        None => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(other) => Err(bad(step, "message", "text", other)),
    }
}

fn read_on_fail(step: usize, value: Option<&Value>) -> Result<OnFail, RecipeError> {
    match value {
        None => Ok(OnFail::Stop),
        Some(Value::String(word)) if word == "stop" => Ok(OnFail::Stop),
        Some(Value::String(word)) if word == "skip" => Ok(OnFail::Skip),
        Some(other) => Err(bad(step, "on_fail", "stop, skip", other)),
    }
}

fn read_retry(step: usize, value: &Value) -> Result<Retry, RecipeError> {
    let Value::Table(table) = value else {
        return Err(bad(step, "retry", "a table `{ while, max }`", value));
    };
    let missing = |key| RecipeError::MissingSetting {
        step,
        kind: StepKind::Check,
        key,
    };
    if let Some(key) = table
        .keys()
        .find(|key| !["while", "max"].contains(&key.as_str()))
    {
        return Err(RecipeError::UnknownSetting {
            step,
            kind: StepKind::Check,
            key: format!("retry.{key}"),
        });
    }
    let while_ = condition(
        step,
        "retry.while",
        table.get("while").ok_or(missing("while"))?,
    )?;
    let max = table.get("max").ok_or(missing("max"))?;
    let max = match max {
        Value::Integer(n) => u8::try_from(*n).ok(),
        _ => None,
    }
    .ok_or_else(|| {
        bad(
            step,
            "retry.max",
            format!("a whole number up to {}", u8::MAX),
            max,
        )
    })?;
    Ok(Retry { while_, max })
}

fn read_choose(
    step: usize,
    kind: StepKind,
    value: Option<&Value>,
) -> Result<Vec<Choice>, RecipeError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let Value::Array(items) = value else {
        return Err(bad(step, "choose", "an array of `{ when, set }`", value));
    };
    items
        .iter()
        .map(|item| {
            let Value::Table(table) = item else {
                return Err(bad(step, "choose", "a table `{ when, set }`", item));
            };
            if let Some(key) = table
                .keys()
                .find(|key| !["when", "set"].contains(&key.as_str()))
            {
                return Err(RecipeError::UnknownSetting {
                    step,
                    kind,
                    key: format!("choose.{key}"),
                });
            }
            let when = table.get("when").ok_or(RecipeError::MissingSetting {
                step,
                kind,
                key: "when",
            })?;
            Ok(Choice {
                when: condition(step, "choose.when", when)?,
                set: read_settings(step, kind, table.get("set"))?,
            })
        })
        .collect()
}

fn read_settings(
    step: usize,
    kind: StepKind,
    value: Option<&Value>,
) -> Result<Settings, RecipeError> {
    let Some(value) = value else {
        return Ok(Settings::new());
    };
    let Value::Table(table) = value else {
        return Err(bad(step, "set", "a table", value));
    };
    table
        .iter()
        .map(|(key, value)| {
            let (_, setting_kind) = kind
                .settings()
                .iter()
                .find(|(name, _)| name == key)
                .ok_or_else(|| RecipeError::UnknownSetting {
                    step,
                    kind,
                    key: key.clone(),
                })?;
            Ok((key.clone(), read_setting(step, key, *setting_kind, value)?))
        })
        .collect()
}

fn read_setting(
    step: usize,
    key: &str,
    kind: SettingKind,
    value: &Value,
) -> Result<Setting, RecipeError> {
    if let Value::String(text) = value
        && let Some(expr) = text.strip_prefix('=')
    {
        return Ok(Setting::Expr(Condition::parse(expr.trim())?));
    }
    let wrong = |expected: String| bad(step, key, expected, value);
    match (kind, value) {
        (SettingKind::Number, value) => number(value)
            .map(Setting::Number)
            .ok_or_else(|| wrong("a number".into())),
        (SettingKind::Bool, Value::Boolean(on)) => Ok(Setting::Bool(*on)),
        (SettingKind::Bool, _) => Err(wrong("true or false".into())),
        (SettingKind::Word(words), Value::String(word)) if words.contains(&word.as_str()) => {
            Ok(Setting::Word(word.clone()))
        }
        (SettingKind::Word(words), _) => Err(wrong(words.join(", "))),
        (SettingKind::Text, Value::String(text)) => Ok(Setting::Text(text.clone())),
        (SettingKind::Text, _) => Err(wrong("text".into())),
        (SettingKind::Points, value) => read_points(value)
            .map(Setting::Points)
            .ok_or_else(|| wrong("a list of [x, y, z], each from 0 to 1".into())),
    }
}

fn read_points(value: &Value) -> Option<Vec<[Scalar; 3]>> {
    let Value::Array(points) = value else {
        return None;
    };
    points
        .iter()
        .map(|point| {
            let Value::Array(parts) = point else {
                return None;
            };
            let parts: Vec<Scalar> = parts.iter().map(number).collect::<Option<_>>()?;
            let point: [Scalar; 3] = parts.try_into().ok()?;
            point
                .iter()
                .all(|c| (0.0..=1.0).contains(c))
                .then_some(point)
        })
        .collect()
}
