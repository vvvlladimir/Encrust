//! The Recipes page: every recipe Prepare for print can run as a table, the built-in ones
//! first; the picked one opened in its form, a built-in one to read and copy (ADR 0405).

mod form;
mod setting;
mod step;

use std::sync::Arc;

use core_recipe::{Listed, Origin, is_built_in};
use egui::{Rect, Ui, pos2};
use printer_profiles::Kind;

use super::supports::Open;
use crate::i18n::tr;
use crate::panels::Window;
use crate::panels::settings::export_text;
use crate::settings::{Settings, delete_recipe, duplicate_recipe, save_recipe};
use crate::status::Status;
use crate::ui::window_frame::{SIDE_PAD, Table, child, scrolled, with_footer};
use crate::ui::{Confirm, Inline, confirm_button, hint, icon, inline_button, quiet_button};

const TABLE: Table = Table {
    heads: || {
        vec![
            tr!("recipes-table-recipe"),
            tr!("recipes-table-kept"),
            tr!("recipes-table-steps"),
        ]
    },
    columns: &[0.0, 0.62, 0.82],
    words: 2,
    lead: 12.0,
};

/// What a click on the page asked for, carried out once the page has been drawn.
enum Action {
    Pick(String),
    Open(String),
    Duplicate(String),
    Export(String),
    Delete(String),
    Save,
}

/// What the header says of the recipe open in the form, while one is.
pub(super) fn editing(settings: &Settings) -> Option<Open> {
    let draft = settings.recipe.as_ref()?;
    Some(Open {
        name: draft.values.name.clone(),
        kind: settings
            .recipe_is_built_in()
            .then(|| tr!("recipes-kept-built-in")),
        reset: None,
    })
}

/// The page, laid out in `content`: the form of the recipe open, or the table.
pub(super) fn page(ui: &mut Ui, content: Rect, window: &mut Window) {
    let listed = window
        .tools
        .prepare
        .library
        .listed(&window.machine.slicing.catalogue);
    let (mut body, mut bar) = with_footer(ui, content);
    let mut action = None;
    if window.machine.settings.recipe.is_some() {
        let whole = body.max_rect();
        scrolled(&mut child(&mut body, whole), |ui| {
            form::form(ui, &mut window.machine.settings);
        });
    } else {
        let top = body.max_rect();
        let rows = Rect::from_min_max(
            pos2(top.left() + SIDE_PAD, top.top() + 8.0),
            pos2(top.right() - SIDE_PAD, top.bottom()),
        );
        let picked = window.machine.settings.recipe_pick.clone();
        table(
            &mut child(&mut body, rows),
            &listed,
            picked.as_deref(),
            &mut action,
        );
    }
    actions(&mut bar, &window.machine.settings, &listed, &mut action);
    if let Some(action) = action {
        apply(window, &listed, action);
    }
}

/// Every recipe: its name, where it is kept, and how many steps it takes; one that would not
/// run says why under the table once picked.
fn table(ui: &mut Ui, listed: &[Listed], picked: Option<&str>, action: &mut Option<Action>) {
    TABLE.head_row(ui);
    let scroll = egui::ScrollArea::vertical().id_salt("recipe-table");
    scroll.auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for entry in listed {
            let kept = match (entry.origin, entry.plan().is_ok()) {
                (Origin::BuiltIn, _) => tr!("recipes-kept-built-in"),
                (Origin::User, true) => tr!("recipes-kept-yours"),
                (Origin::User, false) => tr!("recipes-kept-broken"),
            };
            let steps = entry
                .recipe()
                .map_or_else(|| "—".to_owned(), |recipe| recipe.steps.len().to_string());
            let cells = [entry.name().to_owned(), kept, steps];
            let chosen = picked == Some(entry.id.as_str());
            let response = TABLE.row(ui, entry.name(), chosen, &cells);
            if response.double_clicked() {
                *action = Some(Action::Open(entry.id.clone()));
            } else if response.clicked() {
                *action = Some(Action::Pick(entry.id.clone()));
            }
        }
        let why = picked
            .and_then(|id| listed.iter().find(|entry| entry.id == id))
            .and_then(|entry| entry.plan().err());
        if let Some(why) = why {
            ui.add_space(crate::ui::theme::PANEL_PAD);
            hint(ui, &why.to_string());
        }
    });
}

/// What the bar offers for the recipe picked or open: open it, copy it, write it out, throw
/// a user's away by a second press, and in the form save it once it passes its check.
fn actions(ui: &mut Ui, settings: &Settings, listed: &[Listed], action: &mut Option<Action>) {
    let open = settings.recipe.as_ref();
    let picked = open
        .map(|draft| draft.id.clone())
        .or_else(|| settings.recipe_pick.clone());
    let entry = picked
        .as_deref()
        .and_then(|id| listed.iter().find(|entry| entry.id == id));
    let readable = entry.is_some_and(|entry| entry.recipe().is_some());
    if open.is_none()
        && quiet_button(ui, icon::EDIT, &tr!("profiles-edit"), false, readable).clicked()
    {
        *action = picked.clone().map(Action::Open);
    }
    let unsaved = settings.unsaved();
    let duplicate = quiet_button(
        ui,
        icon::DUPLICATE,
        &tr!("action-duplicate"),
        false,
        readable && !unsaved,
    );
    if duplicate.clicked() {
        *action = picked.clone().map(Action::Duplicate);
    }
    let export = quiet_button(ui, icon::EXPORT, &tr!("common-export"), false, readable);
    if export.clicked() {
        *action = picked.clone().map(Action::Export);
    }
    let own = picked.as_deref().is_some_and(|id| !is_built_in(id));
    if delete_button(ui, picked.is_some(), own) {
        *action = picked.clone().map(Action::Delete);
    }
    if let Some(draft) = open.filter(|_| own) {
        save_button(ui, draft, unsaved, action);
    }
}

/// Delete, by a second press, off with why for a built-in recipe. Answers whether it acted.
fn delete_button(ui: &mut Ui, some: bool, own: bool) -> bool {
    let tooltip = match (some, own) {
        (true, false) => tr!("recipes-built-in-why"),
        _ => tr!("confirm-cannot-undo"),
    };
    let delete = Confirm {
        glyph: icon::REMOVE,
        text: &tr!("profiles-delete"),
        asked: &tr!("confirm-asked-delete"),
        tooltip: &tooltip,
        enabled: own,
    };
    confirm_button(ui, &delete)
}

/// Save, in the accent while there is something to save that passes its check, and why not
/// on hover while it does not.
fn save_button(
    ui: &mut Ui,
    draft: &crate::settings::Draft<crate::settings::RecipeForm>,
    unsaved: bool,
    action: &mut Option<Action>,
) {
    let checked = draft.values.check().err();
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let label = tr!("settings-save");
        let save = Inline::new(&label)
            .primary()
            .enabled(unsaved && checked.is_none());
        let response = inline_button(ui, save);
        let response = match checked {
            Some(_) => response.on_disabled_hover_text(tr!("recipes-fix-first")),
            None => response,
        };
        if response.clicked() {
            *action = Some(Action::Save);
        }
    });
}

fn apply(window: &mut Window, listed: &Arc<Vec<Listed>>, action: Action) {
    let machine = &mut *window.machine;
    let chosen = &mut window.tools.prepare.recipe;
    let find = |id: &str| listed.iter().find(|entry| entry.id == id);
    let outcome = match action {
        Action::Pick(id) => {
            machine.settings.recipe_pick = Some(id);
            Ok(())
        }
        Action::Open(id) => {
            if let Some(entry) = find(&id) {
                machine.settings.open_recipe(entry);
            }
            Ok(())
        }
        Action::Duplicate(id) => match find(&id) {
            Some(entry) => {
                duplicate_recipe(&mut machine.settings, &mut machine.slicing.catalogue, entry)
                    .map(drop)
            }
            None => Ok(()),
        },
        Action::Export(id) => {
            let text = find(&id)
                .and_then(Listed::recipe)
                .map(core_recipe::Recipe::to_toml);
            if let Some(text) = text {
                export_text(machine, Kind::Recipe, &id, &text);
            }
            Ok(())
        }
        Action::Delete(id) => {
            let catalogue = &mut machine.slicing.catalogue;
            delete_recipe(&mut machine.settings, catalogue, &id, chosen)
        }
        Action::Save => save(window).map(drop),
    };
    if let Err(error) = outcome {
        window.machine.status = Status::failed(&error);
    }
}

/// Writes the recipe open in the form; the picker's choice follows a rename. Answers the id
/// it is kept under.
pub(super) fn save(window: &mut Window) -> anyhow::Result<String> {
    let machine = &mut *window.machine;
    let chosen = &mut window.tools.prepare.recipe;
    let id = save_recipe(
        &mut machine.settings,
        &mut machine.slicing.catalogue,
        chosen,
    )?;
    machine.status = Status::Info(tr!("recipes-saved", recipe = id.as_str()));
    Ok(id)
}
