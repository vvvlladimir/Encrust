//! The form of the recipe open: its name and variables, then a block a step with a + before,
//! between and after them to add one there; a built-in recipe is drawn the same, but cannot
//! be changed.

use core_recipe::StepKind;
use egui::Ui;

use super::names::condition_names;
use super::step;
use crate::i18n::tr;
use crate::panels::settings::{form_card, switched_card};
use crate::prepare::kind_name;
use crate::settings::{Checked, RecipeForm, Settings};
use crate::ui::ConditionName;
use crate::ui::{
    DropdownRow, Inline, dropdown_menu_wide, dropdown_row, hint, icon, icon_button, inline_button,
    notice, number_field, small_icon_button, text_box, text_row, theme,
};

/// The recipe open in the form, each block under its title.
pub(super) fn form(ui: &mut Ui, settings: &mut Settings) {
    let built_in = settings.recipe_is_built_in();
    let Some(draft) = settings.recipe.as_mut() else {
        return;
    };
    let checked = draft.values.check().err().unwrap_or_default();
    if built_in {
        ui.add_space(theme::PANEL_PAD);
        let tint = theme::colors().text_mid;
        notice(
            ui,
            icon::PREPARE,
            tint,
            &tr!("recipes-built-in"),
            &tr!("recipes-built-in-how"),
        );
    }
    ui.add_enabled_ui(!built_in, |ui| fields(ui, &mut draft.values, &checked));
}

fn fields(ui: &mut Ui, recipe: &mut RecipeForm, checked: &Checked) {
    form_card(ui, &tr!("recipes-block-recipe"), |ui| {
        crate::ui::describe(ui, &tr!("recipes-block-recipe-help"));
        text_row(ui, &tr!("settings-name"), &mut recipe.name);
        variables(ui, &mut recipe.vars);
    });
    if let Some((None, error)) = &checked.whole {
        let tint = theme::colors().danger;
        notice(
            ui,
            icon::WARNING,
            tint,
            &tr!("recipes-does-not-run"),
            &error.to_string(),
        );
    }
    let names = condition_names(&recipe.vars);
    if recipe.steps.is_empty() {
        ui.add_space(theme::NOTE_GAP);
        ui.vertical_centered(|ui| hint(ui, &tr!("recipes-no-steps")));
    }
    steps(ui, recipe, checked, &names);
}

/// A block a step, each with its switch, a + before, between and after them, and the
/// move, removal or insertion their buttons asked for.
fn steps(ui: &mut Ui, recipe: &mut RecipeForm, checked: &Checked, names: &[ConditionName]) {
    let mut moved = None;
    let mut inserted = add_step(ui, 0);
    let count = recipe.steps.len();
    for index in 0..count {
        let title = tr!(
            "recipe-step-numbered",
            number = index + 1,
            step = kind_name(recipe.steps[index].kind)
        );
        let id = ui.id().with(("recipe-step", index));
        let step = &mut recipe.steps[index];
        let mut on = step.enabled;
        ui.push_id(id, |ui| {
            switched_card(ui, &title, Some(&mut on), |ui| {
                if let Some(asked) = step::fields(ui, step, (index, count), checked, names) {
                    moved = Some((index, asked));
                }
            });
        });
        step.enabled = on;
        inserted = inserted.or(add_step(ui, index + 1));
    }
    if let Some((at, kind)) = inserted {
        recipe.insert_step(at, kind);
    }
    if let Some((index, asked)) = moved {
        match asked {
            step::Asked::Up => recipe.shift(index, true),
            step::Asked::Down => recipe.shift(index, false),
            step::Asked::Remove => {
                recipe.steps.remove(index);
            }
        }
    }
}

/// The recipe's own numbers: a name and a figure a row, a bin to drop one, and a row to
/// add one under them.
fn variables(ui: &mut Ui, vars: &mut Vec<(String, f32)>) {
    let mut dropped = None;
    for (index, (name, value)) in vars.iter_mut().enumerate() {
        ui.push_id(("recipe-var", index), |ui| {
            ui.horizontal(|ui| {
                let field = text_box(egui::TextEdit::singleline(name))
                    .font(theme::code(12.0))
                    .background_color(theme::colors().sunken);
                ui.add_sized(egui::vec2(theme::ROW_NAME_W, theme::FIELD_H), field);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icon_button(ui, icon::REMOVE, &tr!("recipes-var-remove")).clicked() {
                        dropped = Some(index);
                    }
                    let width = ui.available_width();
                    let name = tr!("recipes-var-value");
                    number_field(ui, &name, value, 0.1, f32::MIN..=f32::MAX, Some(2), width);
                });
            });
        });
    }
    if let Some(index) = dropped {
        vars.remove(index);
    }
    let words = tr!("recipes-var-add");
    let add = Inline::new(&words).glyph(icon::ADD);
    if inline_button(ui, add).clicked() {
        vars.push((fresh_var(vars), 0.0));
    }
}

/// A name no variable of the recipe has yet.
fn fresh_var(vars: &[(String, f32)]) -> String {
    (1..)
        .map(|n| format!("var_{n}"))
        .find(|name| vars.iter().all(|(taken, _)| taken != name))
        .unwrap_or_default()
}

/// A + centred on the seam, its list every kind of step; answers the one chosen to stand
/// at `at`.
fn add_step(ui: &mut Ui, at: usize) -> Option<(usize, StepKind)> {
    let response = ui
        .push_id(("recipe-step-add", at), |ui| {
            ui.add_space(theme::NOTE_GAP);
            ui.vertical_centered(|ui| small_icon_button(ui, icon::ADD, &tr!("recipes-step-add")))
                .inner
        })
        .inner;
    dropdown_menu_wide(&response, theme::BLOCK_TITLE_W, |ui| {
        StepKind::ALL.into_iter().find(|kind| {
            let name = kind_name(*kind);
            dropdown_row(ui, &DropdownRow::named(&name, false)).clicked()
        })
    })
    .flatten()
    .map(|kind| (at, kind))
}
