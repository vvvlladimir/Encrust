//! A recipe as the Recipes page edits it: every condition kept as typed, so a draft that does
//! not read yet stays in the form, and checked whole before it may be saved (ADR 0405).

use std::collections::BTreeMap;

use core_recipe::{
    Choice, Condition, OnFail, Plan, Recipe, RecipeError, Retry, Setting, SettingKind, Settings,
    Step, StepKind, validate,
};

#[cfg(test)]
mod tests;

/// The recipe in the form.
#[derive(Debug, Clone, PartialEq)]
pub struct RecipeForm {
    pub name: String,
    pub vars: Vec<(String, f32)>,
    pub steps: Vec<StepForm>,
}

/// One step in the form, its conditions kept as typed.
#[derive(Debug, Clone, PartialEq)]
pub struct StepForm {
    pub kind: StepKind,
    pub enabled: bool,
    /// The condition as typed; empty runs always.
    pub when: String,
    pub on_fail: OnFail,
    pub set: SettingsForm,
    pub choose: Vec<ChoiceForm>,
    pub message: String,
    pub stop_if: String,
    pub retry: Option<(String, u8)>,
}

/// A rule of `choose`: its condition as typed and the settings it lays over the step's.
#[derive(Debug, Clone, PartialEq)]
pub struct ChoiceForm {
    pub when: String,
    pub set: SettingsForm,
}

/// A step's or a rule's settings by key.
pub type SettingsForm = BTreeMap<String, SettingForm>;

/// One setting as the form holds it: a value, or an expression as typed, without its `=`.
#[derive(Debug, Clone, PartialEq)]
pub enum SettingForm {
    Number(f32),
    Bool(bool),
    Word(String),
    Text(String),
    Points(Vec<[f32; 3]>),
    Expr(String),
}

impl SettingForm {
    /// What a setting of `kind` starts at when it is added: the engine's default where the
    /// form can say it, else the first word or nothing.
    pub fn fresh(kind: SettingKind) -> Self {
        match kind {
            SettingKind::Number => Self::Number(0.0),
            SettingKind::Bool => Self::Bool(true),
            SettingKind::Word(words) => Self::Word(words.first().copied().unwrap_or("").into()),
            SettingKind::Text => Self::Text(String::new()),
            SettingKind::Points => Self::Points(vec![[0.5, 0.5, 0.0]]),
        }
    }
}

/// Where in the form a condition is typed, which its error stands under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Field {
    When(usize),
    StopIf(usize),
    RetryWhile(usize),
    ChoiceWhen(usize, usize),
    /// A setting's expression: its step, the rule it is in if any, and its key.
    Setting(usize, Option<usize>, String),
}

/// What is wrong with the form: the conditions that do not read, by field, and the first
/// error of the recipe as a whole, with the step it names where it names one.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Checked {
    pub fields: Vec<(Field, RecipeError)>,
    pub whole: Option<(Option<usize>, RecipeError)>,
}

impl Checked {
    /// Why the condition typed at `field` does not read, if it does not.
    pub fn at(&self, field: &Field) -> Option<&RecipeError> {
        self.fields
            .iter()
            .find(|(at, _)| at == field)
            .map(|(_, error)| error)
    }
}

impl RecipeForm {
    /// `recipe` as the form holds it.
    pub fn of(recipe: &Recipe) -> Self {
        Self {
            name: recipe.name.clone(),
            vars: recipe
                .vars
                .iter()
                .map(|(name, n)| (name.clone(), *n))
                .collect(),
            steps: recipe.steps.iter().map(StepForm::of).collect(),
        }
    }

    /// The recipe the form holds, checked as the file it is written to would be, or what is
    /// wrong with it.
    pub fn check(&self) -> Result<Plan, Checked> {
        let mut checked = Checked::default();
        let recipe = self.recipe(&mut checked);
        if !checked.fields.is_empty() {
            return Err(checked);
        }
        // Through its text, so a word, a point or a key is held to what a file may say.
        let read = Recipe::from_toml(&recipe.to_toml()).and_then(validate);
        read.map_err(|error| Checked {
            whole: Some((step_of(&error), error)),
            ..checked
        })
    }

    fn recipe(&self, checked: &mut Checked) -> Recipe {
        Recipe {
            name: self.name.trim().to_owned(),
            vars: self
                .vars
                .iter()
                .map(|(name, n)| (name.trim().to_owned(), *n))
                .collect(),
            steps: (0..self.steps.len())
                .map(|index| self.steps[index].step(index, checked))
                .collect(),
        }
    }

    /// Moves step `index` one place up, or down; at either end it stays.
    pub fn shift(&mut self, index: usize, up: bool) {
        let to = if up {
            index.checked_sub(1)
        } else {
            Some(index + 1)
        };
        if let Some(to) = to.filter(|to| *to < self.steps.len()) {
            self.steps.swap(index, to);
        }
    }

    /// A new step of `kind` at `at`, past the end at the end, running always with the
    /// engine's defaults.
    pub fn insert_step(&mut self, at: usize, kind: StepKind) {
        let at = at.min(self.steps.len());
        self.steps.insert(
            at,
            StepForm {
                kind,
                enabled: true,
                when: String::new(),
                on_fail: OnFail::Stop,
                set: SettingsForm::new(),
                choose: Vec::new(),
                message: String::new(),
                stop_if: String::new(),
                retry: None,
            },
        );
    }
}

impl StepForm {
    fn of(step: &Step) -> Self {
        Self {
            kind: step.kind,
            enabled: step.enabled,
            when: step
                .when
                .as_ref()
                .map(|c| c.text().to_owned())
                .unwrap_or_default(),
            on_fail: step.on_fail,
            set: settings_form(&step.set),
            choose: step
                .choose
                .iter()
                .map(|choice| ChoiceForm {
                    when: choice.when.text().to_owned(),
                    set: settings_form(&choice.set),
                })
                .collect(),
            message: step.message.clone().unwrap_or_default(),
            stop_if: step
                .stop_if
                .as_ref()
                .map(|c| c.text().to_owned())
                .unwrap_or_default(),
            retry: step
                .retry
                .as_ref()
                .map(|r| (r.while_.text().to_owned(), r.max)),
        }
    }

    fn step(&self, index: usize, checked: &mut Checked) -> Step {
        let mut read = |text: &str, field: Field| -> Option<Condition> {
            let text = text.trim();
            if text.is_empty() {
                return None;
            }
            Condition::parse(text)
                .map_err(|error| checked.fields.push((field, error)))
                .ok()
        };
        let when = read(&self.when, Field::When(index));
        let stop_if = read(&self.stop_if, Field::StopIf(index));
        let retry = self.retry.as_ref().and_then(|(text, max)| {
            let while_ = read(text, Field::RetryWhile(index))?;
            Some(Retry { while_, max: *max })
        });
        let set = settings(&self.set, index, None, &mut read);
        let choose = self
            .choose
            .iter()
            .enumerate()
            .filter_map(|(at, choice)| {
                let when = read(&choice.when, Field::ChoiceWhen(index, at))?;
                let set = settings(&choice.set, index, Some(at), &mut read);
                Some(Choice { when, set })
            })
            .collect();
        let message = Some(self.message.trim().to_owned()).filter(|m| !m.is_empty());
        let says = matches!(self.kind, StepKind::Gate | StepKind::Pause);
        Step {
            kind: self.kind,
            enabled: self.enabled,
            when,
            set,
            choose,
            on_fail: self.on_fail,
            message: message.filter(|_| says),
            stop_if: stop_if.filter(|_| self.kind == StepKind::Gate),
            retry: retry.filter(|_| self.kind == StepKind::Check),
        }
    }
}

fn settings(
    form: &SettingsForm,
    step: usize,
    choice: Option<usize>,
    read: &mut impl FnMut(&str, Field) -> Option<Condition>,
) -> Settings {
    form.iter()
        .filter_map(|(key, setting)| {
            let setting = match setting {
                SettingForm::Number(n) => Setting::Number(*n),
                SettingForm::Bool(on) => Setting::Bool(*on),
                SettingForm::Word(word) => Setting::Word(word.clone()),
                SettingForm::Text(text) => Setting::Text(text.clone()),
                SettingForm::Points(points) => Setting::Points(points.clone()),
                SettingForm::Expr(text) => {
                    let field = Field::Setting(step, choice, key.clone());
                    // An empty expression is one not typed yet: it reads as nothing.
                    let condition = read(text, field.clone());
                    if condition.is_none() && text.trim().is_empty() {
                        return None;
                    }
                    Setting::Expr(condition?)
                }
            };
            Some((key.clone(), setting))
        })
        .collect()
}

fn settings_form(settings: &Settings) -> SettingsForm {
    settings
        .iter()
        .map(|(key, setting)| {
            let form = match setting {
                Setting::Number(n) => SettingForm::Number(*n),
                Setting::Bool(on) => SettingForm::Bool(*on),
                Setting::Word(word) => SettingForm::Word(word.clone()),
                Setting::Text(text) => SettingForm::Text(text.clone()),
                Setting::Points(points) => SettingForm::Points(points.clone()),
                Setting::Expr(condition) => SettingForm::Expr(condition.text().to_owned()),
            };
            (key.clone(), form)
        })
        .collect()
}

/// The step an error of the whole recipe names, where it names one.
fn step_of(error: &RecipeError) -> Option<usize> {
    match error {
        RecipeError::UnknownStep { step, .. }
        | RecipeError::UnknownSetting { step, .. }
        | RecipeError::BadSetting { step, .. }
        | RecipeError::MissingSetting { step, .. }
        | RecipeError::NotYetKnown { step, .. }
        | RecipeError::RetryTooMany { step, .. }
        | RecipeError::NoSuchStep { step } => Some(*step),
        _ => None,
    }
}
