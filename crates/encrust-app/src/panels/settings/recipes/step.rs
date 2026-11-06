//! One step of the recipe in the form: where it stands, its condition, what a failure does,
//! its settings and the rules that choose others, and what a gate, a pause or a check adds.

use core_recipe::{MAX_RETRY, OnFail, StepKind};
use egui::Ui;

use super::setting::settings_rows;
use crate::i18n::tr;
use crate::settings::{Checked, ChoiceForm, Field, SettingsForm, StepForm};
use crate::ui::{
    ConditionName, ConditionRow, Inline, Segment, Segmented, condition_row, count_row, field_error,
    icon, icon_button, inline_button, nested, row_name, subheading, switch, text_row,
};

/// What the step's own buttons asked of the list.
pub(super) enum Asked {
    Up,
    Down,
    Remove,
}

/// The fields of step `index` of `count`, and what its buttons asked; `names` are what its
/// conditions can read.
pub(super) fn fields(
    ui: &mut Ui,
    step: &mut StepForm,
    (index, count): (usize, usize),
    checked: &Checked,
    names: &[ConditionName],
) -> Option<Asked> {
    crate::ui::describe(ui, &kind_help(step.kind));
    crate::ui::describe(ui, &tr!("recipes-step-help"));
    crate::ui::describe(ui, &tr!("recipes-conditions-help"));
    let asked = buttons(ui, index, count);
    if let Some((Some(at), error)) = &checked.whole
        && *at == index
    {
        field_error(ui, &error.to_string());
    }
    let why = checked.at(&Field::When(index)).map(ToString::to_string);
    let when = ConditionRow {
        label: &tr!("recipes-when"),
        error: why.as_deref(),
        empty: &tr!("recipes-when-empty"),
        names,
    };
    condition_row(ui, &when, &mut step.when);
    on_fail(ui, &mut step.on_fail);
    settings_rows(
        ui,
        step.kind,
        &mut step.set,
        (index, None),
        (checked, names),
    );
    own_fields(ui, step, index, (checked, names));
    if !step.kind.settings().is_empty() {
        choose(ui, step.kind, &mut step.choose, index, (checked, names));
    }
    asked
}

/// What a step of `kind` does to the model, in a sentence or two.
fn kind_help(kind: StepKind) -> String {
    match kind {
        StepKind::Repair => tr!("recipes-step-help-repair"),
        StepKind::Orient => tr!("recipes-step-help-orient"),
        StepKind::Hollow => tr!("recipes-step-help-hollow"),
        StepKind::Drain => tr!("recipes-step-help-drain"),
        StepKind::Supports => tr!("recipes-step-help-supports"),
        StepKind::Check => tr!("recipes-step-help-check"),
        StepKind::Arrange => tr!("recipes-step-help-arrange"),
        StepKind::Save => tr!("recipes-step-help-save"),
        StepKind::Gate => tr!("recipes-step-help-gate"),
        StepKind::Pause => tr!("recipes-step-help-pause"),
    }
}

/// What a gate, a pause or a check adds to the step's fields.
fn own_fields(
    ui: &mut Ui,
    step: &mut StepForm,
    index: usize,
    (checked, names): (&Checked, &[ConditionName]),
) {
    let error = |field: Field| checked.at(&field).map(ToString::to_string);
    match step.kind {
        StepKind::Gate => {
            let why = error(Field::StopIf(index));
            let stop_if = ConditionRow {
                label: &tr!("recipes-stop-if"),
                error: why.as_deref(),
                empty: &tr!("recipes-stop-if-empty"),
                names,
            };
            condition_row(ui, &stop_if, &mut step.stop_if);
            text_row(ui, &tr!("recipes-message"), &mut step.message);
        }
        StepKind::Pause => {
            text_row(ui, &tr!("recipes-message"), &mut step.message);
        }
        StepKind::Check => {
            let why = error(Field::RetryWhile(index));
            retry(ui, &mut step.retry, why.as_deref(), names);
        }
        _ => {}
    }
}

/// Move up, move down and remove, at the right of the step's first row.
fn buttons(ui: &mut Ui, index: usize, count: usize) -> Option<Asked> {
    let mut asked = None;
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if icon_button(ui, icon::REMOVE, &tr!("recipes-step-remove")).clicked() {
            asked = Some(Asked::Remove);
        }
        let down = ui.add_enabled_ui(index + 1 < count, |ui| {
            icon_button(ui, icon::MOVE_DOWN, &tr!("recipes-step-down"))
        });
        if down.inner.clicked() {
            asked = Some(Asked::Down);
        }
        let up = ui.add_enabled_ui(index > 0, |ui| {
            icon_button(ui, icon::MOVE_UP, &tr!("recipes-step-up"))
        });
        if up.inner.clicked() {
            asked = Some(Asked::Up);
        }
    });
    asked
}

fn on_fail(ui: &mut Ui, on_fail: &mut OnFail) {
    ui.horizontal(|ui| {
        row_name(ui, &tr!("recipes-on-fail"));
        let segments = [
            Segment::new(OnFail::Stop, tr!("recipes-on-fail-stop")),
            Segment::new(OnFail::Skip, tr!("recipes-on-fail-skip")),
        ];
        Segmented::new(&segments)
            .width(ui.available_width())
            .show(ui, on_fail);
    });
}

/// A check's retry: supports the islands again while its condition holds, at most a few times.
fn retry(
    ui: &mut Ui,
    retry: &mut Option<(String, u8)>,
    error: Option<&str>,
    names: &[ConditionName],
) {
    let mut on = retry.is_some();
    if switch(ui, &mut on, &tr!("recipes-retry")).changed() {
        *retry = on.then(|| ("islands > 0".to_owned(), 2));
    }
    if let Some((while_, max)) = retry {
        nested(ui, |ui| {
            let row = ConditionRow {
                label: &tr!("recipes-retry-while"),
                error,
                empty: &tr!("recipes-retry-while-empty"),
                names,
            };
            condition_row(ui, &row, while_);
            let mut times = u32::from(*max);
            let at_most = tr!("recipes-retry-max");
            if count_row(ui, &at_most, &mut times, "", 0.05, 1..=u32::from(MAX_RETRY)) {
                *max = u8::try_from(times).unwrap_or(MAX_RETRY);
            }
        });
    }
}

/// The rules that lay other settings over the step's where their condition holds, the
/// first that holds taken.
fn choose(
    ui: &mut Ui,
    kind: StepKind,
    rules: &mut Vec<ChoiceForm>,
    index: usize,
    (checked, names): (&Checked, &[ConditionName]),
) {
    subheading(ui, &tr!("recipes-choose"));
    let mut dropped = None;
    for (at, rule) in rules.iter_mut().enumerate() {
        ui.push_id(("rule", at), |ui| {
            nested(ui, |ui| {
                let why = checked
                    .at(&Field::ChoiceWhen(index, at))
                    .map(ToString::to_string);
                let when = ConditionRow {
                    label: &tr!("recipes-when"),
                    error: why.as_deref(),
                    empty: &tr!("recipes-rule-when-empty"),
                    names,
                };
                condition_row(ui, &when, &mut rule.when);
                settings_rows(ui, kind, &mut rule.set, (index, Some(at)), (checked, names));
                let words = tr!("recipes-rule-remove");
                let remove = Inline::new(&words).glyph(icon::REMOVE);
                if inline_button(ui, remove).clicked() {
                    dropped = Some(at);
                }
            });
        });
    }
    if let Some(at) = dropped {
        rules.remove(at);
    }
    let words = tr!("recipes-rule-add");
    let add = Inline::new(&words).glyph(icon::ADD);
    if inline_button(ui, add).clicked() {
        rules.push(ChoiceForm {
            when: String::new(),
            set: SettingsForm::new(),
        });
    }
}
