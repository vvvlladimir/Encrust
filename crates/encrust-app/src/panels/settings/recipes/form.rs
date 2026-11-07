//! The form of the recipe open: its name and variables, then its steps as a chain; a
//! built-in recipe is drawn the same, but only folds.

use egui::Ui;

use super::chain;
use super::names::condition_names;
use crate::i18n::tr;
use crate::panels::settings::form_card;
use crate::settings::{Checked, RecipeForm, Settings};
use crate::ui::{
    Inline, hint, icon, icon_button, inline_button, notice, number_field, text_box, text_row, theme,
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
    let folds = chain::folds(&draft.id);
    fields(ui, &mut draft.values, &checked, (folds, !built_in));
}

fn fields(
    ui: &mut Ui,
    recipe: &mut RecipeForm,
    checked: &Checked,
    (folds, editable): (egui::Id, bool),
) {
    ui.add_enabled_ui(editable, |ui| {
        form_card(ui, &tr!("recipes-block-recipe"), |ui| {
            crate::ui::describe(ui, &tr!("recipes-block-recipe-help"));
            text_row(ui, &tr!("settings-name"), &mut recipe.name);
            variables(ui, &mut recipe.vars);
        });
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
    ui.add_space(theme::NOTE_GAP);
    chain::chain(ui, recipe, (checked, &names), folds, editable);
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
