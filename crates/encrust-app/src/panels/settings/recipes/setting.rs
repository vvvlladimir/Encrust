//! A step's settings by `StepKind::settings`: a setting left out takes the engine's default
//! and can be set; one set takes its field, a number or a switch its expression instead.

use core_recipe::{SettingKind, StepKind};
use egui::{Align, Layout, Ui};

use crate::i18n::tr;
use crate::settings::{Checked, Field, SettingForm, SettingsForm};
use crate::ui::{
    DropdownFace, DropdownRow, Inline, condition_row, dropdown, dropdown_menu, dropdown_row, hint,
    icon, icon_button, icon_toggle, inline_button, number_row, row_name, switch, text_row, theme,
    unit_field,
};

/// The settings of a step of `kind`, or of one of its rules: `at` is the step and the rule.
pub(super) fn settings_rows(
    ui: &mut Ui,
    kind: StepKind,
    set: &mut SettingsForm,
    at: (usize, Option<usize>),
    checked: &Checked,
) {
    for (key, setting_kind) in kind.settings() {
        let label = setting_name(key);
        let field = Field::Setting(at.0, at.1, (*key).to_owned());
        let error = checked.at(&field).map(ToString::to_string);
        ui.push_id(*key, |ui| {
            let held = set.get(*key).map(|s| matches!(s, SettingForm::Expr(_)));
            let asked = with_buttons(ui, held, *setting_kind, |ui| match set.get_mut(*key) {
                None => unset(ui, &label),
                Some(setting) => {
                    value_row(ui, &label, key, *setting_kind, setting, error.as_deref())
                }
            });
            match asked {
                Some(Button::Set) => {
                    set.insert((*key).to_owned(), SettingForm::fresh(*setting_kind));
                }
                Some(Button::Default) => {
                    set.remove(*key);
                }
                Some(Button::Expression) => {
                    if let Some(setting) = set.get_mut(*key) {
                        *setting = flipped(setting, *setting_kind);
                    }
                }
                None => {}
            }
        });
    }
}

/// What a setting's own buttons asked.
enum Button {
    Set,
    Default,
    Expression,
}

/// The row `add` lays out, with its buttons at its right: Set while it is left out (`held`
/// none), else back to the default, and for a number or a switch the expression in its place,
/// lit while `held` says it stands.
fn with_buttons(
    ui: &mut Ui,
    held: Option<bool>,
    kind: SettingKind,
    add: impl FnOnce(&mut Ui),
) -> Option<Button> {
    let mut asked = None;
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            match held {
                None => {
                    if icon_button(ui, icon::ADD, &tr!("recipes-setting-set")).clicked() {
                        asked = Some(Button::Set);
                    }
                }
                Some(on) => {
                    if icon_button(ui, icon::RESET, &tr!("recipes-setting-default")).clicked() {
                        asked = Some(Button::Default);
                    }
                    if matches!(kind, SettingKind::Number | SettingKind::Bool) {
                        let tip = tr!("recipes-setting-expression");
                        if icon_toggle(ui, icon::EXPRESSION, &tip, on).clicked() {
                            asked = Some(Button::Expression);
                        }
                    }
                }
            }
            ui.with_layout(Layout::top_down(Align::Min), add);
        });
    });
    asked
}

/// A setting left out: its name, and that the engine's default stands.
fn unset(ui: &mut Ui, label: &str) {
    ui.horizontal(|ui| {
        row_name(ui, label);
        hint(ui, &tr!("recipes-setting-left-out"));
    });
}

fn value_row(
    ui: &mut Ui,
    label: &str,
    key: &str,
    kind: SettingKind,
    setting: &mut SettingForm,
    error: Option<&str>,
) {
    match (setting, kind) {
        (SettingForm::Number(value), _) => {
            let (unit, decimals) = unit_of(key);
            number_row(ui, label, value, &unit, 0.1, f32::MIN..=f32::MAX, decimals);
        }
        (SettingForm::Bool(on), _) => {
            switch(ui, on, label);
        }
        (SettingForm::Word(word), SettingKind::Word(words)) => word_row(ui, label, word, words),
        (SettingForm::Word(text) | SettingForm::Text(text), _) => {
            text_row(ui, label, text);
        }
        (SettingForm::Points(points), _) => points_rows(ui, label, points),
        (SettingForm::Expr(text), _) => {
            condition_row(ui, label, text, error);
        }
    }
}

/// One of a setting's words, from a list of them.
fn word_row(ui: &mut Ui, label: &str, word: &mut String, words: &[&str]) {
    ui.horizontal(|ui| {
        row_name(ui, label);
        let face = DropdownFace {
            lead: None,
            ..DropdownFace::plain("", word)
        };
        let response = dropdown(ui, &face, ui.available_width());
        dropdown_menu(&response, |ui| {
            for choice in words {
                let row = DropdownRow::named(choice, *choice == word.as_str());
                if dropdown_row(ui, &row).clicked() {
                    *word = (*choice).to_owned();
                }
            }
        });
    });
}

/// Points as rows of three fractions of the placed box, each dropped by its bin, and a row
/// to add one under them.
fn points_rows(ui: &mut Ui, label: &str, points: &mut Vec<[f32; 3]>) {
    row_name(ui, label);
    let mut dropped = None;
    let names = ["X", "Y", "Z"];
    for (index, point) in points.iter_mut().enumerate() {
        ui.push_id(("point", index), |ui| {
            ui.horizontal(|ui| {
                let width = (ui.available_width() - theme::ICON_SIZE) / 3.0 - theme::ITEM_GAP;
                for (axis, value) in point.iter_mut().enumerate() {
                    unit_field(ui, names[axis], value, "", 0.01, 0.0..=1.0, Some(2), width);
                }
                if icon_button(ui, icon::REMOVE, &tr!("recipes-point-remove")).clicked() {
                    dropped = Some(index);
                }
            });
        });
    }
    if let Some(index) = dropped {
        points.remove(index);
    }
    let words = tr!("recipes-point-add");
    let add = Inline::new(&words).glyph(icon::ADD);
    if inline_button(ui, add).clicked() {
        points.push([0.5, 0.5, 0.0]);
    }
}

/// A number or a switch as an expression, or an expression back to a value.
fn flipped(setting: &SettingForm, kind: SettingKind) -> SettingForm {
    match setting {
        SettingForm::Expr(_) => SettingForm::fresh(kind),
        SettingForm::Number(n) => SettingForm::Expr(format!("{n}")),
        SettingForm::Bool(on) => SettingForm::Expr(on.to_string()),
        other => other.clone(),
    }
}

/// The unit a key names by its ending, and the decimals the standard gives it.
fn unit_of(key: &str) -> (String, usize) {
    if key.ends_with("_mm") {
        (tr!("unit-mm"), 2)
    } else if key.ends_with("_deg") {
        (tr!("unit-deg"), 1)
    } else {
        (String::new(), 2)
    }
}

/// What a setting is called in the form, by its key in the file.
fn setting_name(key: &str) -> String {
    match key {
        "method" => tr!("recipes-set-method"),
        "goal" => tr!("recipes-set-goal"),
        "max_tilt_deg" => tr!("recipes-set-max-tilt"),
        "wall_mm" => tr!("recipes-set-wall"),
        "mode" => tr!("recipes-set-mode"),
        "infill" => tr!("recipes-set-infill"),
        "keep_solid_under_mm" => tr!("recipes-set-keep-solid"),
        "at" => tr!("recipes-set-at"),
        "points" => tr!("recipes-set-points"),
        "diameter_mm" => tr!("recipes-set-diameter"),
        "depth_mm" => tr!("recipes-set-depth"),
        "vent" => tr!("recipes-set-vent"),
        "plugs" => tr!("recipes-set-plugs"),
        "profile" => tr!("recipes-set-profile"),
        "cavity" => tr!("recipes-set-cavity"),
        "spacing_mm" => tr!("recipes-set-spacing"),
        "folder" => tr!("recipes-set-folder"),
        "name" => tr!("recipes-set-name"),
        "send" => tr!("recipes-set-send"),
        other => other.to_owned(),
    }
}
