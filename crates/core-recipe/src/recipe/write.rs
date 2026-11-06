//! A `Recipe` into the text `from_toml` reads back.

use core_geometry::Scalar;
use toml::{Table, Value};

use super::{OnFail, Recipe, Setting, Settings, Step};

pub(super) fn recipe_to_toml(recipe: &Recipe) -> String {
    let mut table = Table::new();
    table.insert("name".into(), Value::String(recipe.name.clone()));
    table.insert("format".into(), Value::Integer(i64::from(super::FORMAT)));
    if !recipe.vars.is_empty() {
        let vars = recipe
            .vars
            .iter()
            .map(|(key, n)| (key.clone(), float(*n)))
            .collect();
        table.insert("vars".into(), Value::Table(vars));
    }
    let steps = recipe.steps.iter().map(step_value).collect();
    table.insert("step".into(), Value::Array(steps));
    toml::to_string(&table).unwrap_or_default()
}

fn step_value(step: &Step) -> Value {
    let mut table = Table::new();
    table.insert("do".into(), Value::String(step.kind.name().into()));
    if !step.enabled {
        table.insert("enabled".into(), Value::Boolean(false));
    }
    if step.on_fail == OnFail::Skip {
        table.insert("on_fail".into(), Value::String("skip".into()));
    }
    if let Some(when) = &step.when {
        table.insert("when".into(), Value::String(when.text().into()));
    }
    if let Some(stop_if) = &step.stop_if {
        table.insert("stop_if".into(), Value::String(stop_if.text().into()));
    }
    if let Some(message) = &step.message {
        table.insert("message".into(), Value::String(message.clone()));
    }
    if !step.set.is_empty() {
        table.insert("set".into(), settings_value(&step.set));
    }
    if !step.choose.is_empty() {
        let choices = step
            .choose
            .iter()
            .map(|choice| {
                let mut entry = Table::new();
                entry.insert("when".into(), Value::String(choice.when.text().into()));
                entry.insert("set".into(), settings_value(&choice.set));
                Value::Table(entry)
            })
            .collect();
        table.insert("choose".into(), Value::Array(choices));
    }
    if let Some(retry) = &step.retry {
        let mut entry = Table::new();
        entry.insert("while".into(), Value::String(retry.while_.text().into()));
        entry.insert("max".into(), Value::Integer(i64::from(retry.max)));
        table.insert("retry".into(), Value::Table(entry));
    }
    Value::Table(table)
}

fn settings_value(settings: &Settings) -> Value {
    Value::Table(
        settings
            .iter()
            .map(|(key, setting)| (key.clone(), setting_value(setting)))
            .collect(),
    )
}

fn setting_value(setting: &Setting) -> Value {
    match setting {
        Setting::Number(n) => float(*n),
        Setting::Bool(on) => Value::Boolean(*on),
        Setting::Word(text) | Setting::Text(text) => Value::String(text.clone()),
        Setting::Points(points) => Value::Array(
            points
                .iter()
                .map(|point| {
                    Value::Array(point.iter().map(|c| float(*c)).collect())
                })
                .collect(),
        ),
        Setting::Expr(condition) => Value::String(format!("= {}", condition.text())),
    }
}

/// `n` as the shortest decimal that reads back to it, `0.8` and not the `0.800000011920929`
/// its widening to `f64` would write.
fn float(n: Scalar) -> Value {
    Value::Float(n.to_string().parse().unwrap_or(f64::from(n)))
}
