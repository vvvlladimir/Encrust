//! Prepare for print over the plate card: the recipe and the run, bare on the stage; while a
//! run goes, its step, share done and Cancel on a card, as a job notice says them (ADR 0406).

use core_recipe::{Listed, Origin};

use crate::i18n::tr;
use crate::panels::Window;
use crate::settings::Section;
use crate::ui::{
    DropdownFace, DropdownRow, Lead, companion_square, dropdown, dropdown_menu_wide, dropdown_row,
    group_name, hairline, icon, job_lines, picker_foot, theme,
};

/// The row, as wide as the room it is drawn in.
pub(super) fn ui(ui: &mut egui::Ui, window: &mut Window) {
    if let Some((step, share)) = window.tools.prepare.progress() {
        if job_lines(ui, &step.label(), Some(share), false) {
            crate::prepare::cancel(window.doc, window.tools, window.machine);
        }
        return;
    }
    let row_w = ui.available_width();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme::NOTE_GAP;
        let width = (row_w - theme::BUTTON_H - theme::NOTE_GAP).max(0.0);
        recipe(ui, window, width, row_w);
        run(ui, window);
    });
}

/// Starts Prepare for print, and says on the stage why when it cannot.
pub(crate) fn start(window: &mut Window) {
    let started = crate::prepare::start(window.doc, window.tools, window.machine);
    if let Err(error) = started {
        window.machine.status = crate::status::Status::failed(&error);
    }
}

/// The run, off with why on its hover while it cannot start.
fn run(ui: &mut egui::Ui, window: &mut Window) {
    let blocked = crate::prepare::blocker(window.doc, window.tools, window.machine);
    let enabled = blocked.is_none();
    let tip =
        blocked.unwrap_or_else(|| tr!("prepare-row-tooltip", scope = window.doc.scene.scope()));
    if companion_square(ui, icon::PLAY, &tip, theme::BUTTON_H, enabled).clicked() {
        start(window);
    }
}

/// The picker of recipes, `width` points wide, its list as wide as the row: the built-in
/// ones, then the user's.
fn recipe(ui: &mut egui::Ui, window: &mut Window, width: f32, row_w: f32) {
    let listed = window
        .tools
        .prepare
        .library
        .listed(&window.machine.slicing.catalogue);
    let chosen = &window.tools.prepare.recipe;
    let name = listed
        .iter()
        .find(|entry| entry.id == *chosen)
        .map_or(chosen.as_str(), Listed::name);
    let face = DropdownFace::plain(icon::PREPARE, name);
    let response = dropdown(ui, &face, width).on_hover_text(tr!("prepare-row-recipe"));
    let (choice, edit) = menu(&response, row_w, &listed, chosen).unwrap_or_default();
    if let Some(id) = choice {
        window.tools.prepare.recipe = id;
    }
    if edit {
        open_recipes(window);
    }
}

/// The list under the picker: the built-in recipes, then the user's, a recipe that cannot
/// run led by the warning glyph, and at its foot the way to Settings. Answers the recipe
/// chosen and whether the foot was pressed, while it is open.
fn menu(
    response: &egui::Response,
    width: f32,
    listed: &[Listed],
    chosen: &str,
) -> Option<(Option<String>, bool)> {
    dropdown_menu_wide(response, width, |ui| {
        let mut choice = None;
        for (origin, title) in [
            (Origin::BuiltIn, tr!("prepare-row-built-in")),
            (Origin::User, tr!("prepare-row-yours")),
        ] {
            let group: Vec<&Listed> = listed.iter().filter(|e| e.origin == origin).collect();
            if group.is_empty() {
                continue;
            }
            group_name(ui, &title);
            for entry in group {
                let lead = entry.plan().is_err().then_some(Lead::Glyph(icon::WARNING));
                let row = DropdownRow {
                    lead,
                    name: entry.name(),
                    aside: None,
                    chosen: entry.id == chosen,
                };
                if dropdown_row(ui, &row).clicked() {
                    choice = Some(entry.id.clone());
                }
            }
        }
        hairline(ui);
        let edit = picker_foot(ui, icon::PROFILES, &tr!("prepare-row-edit-recipes"));
        (choice, edit.clicked())
    })
}

/// Opens Settings on the Recipes page, the recipe picked here picked there.
fn open_recipes(window: &mut Window) {
    let settings = &mut window.machine.settings;
    if !settings.open {
        crate::panels::toggle_settings(window.machine);
    }
    let settings = &mut window.machine.settings;
    if settings.leave(crate::settings::Leaving::Turn(Section::Recipes)) {
        settings.recipe_pick = Some(window.tools.prepare.recipe.clone());
    }
}
