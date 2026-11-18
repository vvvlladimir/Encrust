//! The Edit menu: history, the selection, the plate, and Find.

use super::{action_row, clicked};
use crate::i18n::tr;
use crate::panels::Window;
use crate::scene::ObjectId;
use crate::shortcuts::{self, Action};
use crate::state::Doc;
use crate::ui::{MenuRow, icon, menu_row, menu_rule, menu_sub};

/// History, the selection, the plate, and Find.
pub(super) fn edit_menu(ui: &mut egui::Ui, window: &mut Window) {
    let history = &window.doc.history;
    let (can_undo, can_redo) = (history.can_undo(), history.can_redo());
    let (undo, redo) = (tr!("action-undo"), tr!("action-redo"));
    let undo = action_row(Some(icon::UNDO), &undo, Action::Undo).enabled(can_undo);
    if clicked(ui, &undo) {
        window.doc.history.undo(
            &mut window.doc.scene,
            window.tools,
            &mut window.machine.slicing,
        );
    }
    let redo = action_row(Some(icon::REDO), &redo, Action::Redo).enabled(can_redo);
    if clicked(ui, &redo) {
        window.doc.history.redo(
            &mut window.doc.scene,
            window.tools,
            &mut window.machine.slicing,
        );
    }
    menu_rule(ui);
    selection_items(ui, window);
    menu_rule(ui);
    plate_items(ui, window);
    menu_rule(ui);
    if clicked(
        ui,
        &action_row(Some(icon::FIND), &tr!("action-find"), Action::Find),
    ) {
        window.view.options.find = true;
    }
}

/// Every model, none, a copy, gone, and another plate. The first that is off says why.
fn selection_items(ui: &mut egui::Ui, window: &mut Window) {
    let anything = window
        .doc
        .scene
        .has_printable(window.doc.scene.active_plate());
    let all = tr!("action-select-all");
    let all = action_row(Some(icon::SELECT_ALL), &all, Action::SelectAll).enabled(anything);
    if clicked(ui, &all) {
        window.doc.scene.select_here();
    }
    let picked = !window.doc.scene.selection().is_empty();
    let none = tr!("action-deselect");
    let none = action_row(None, &none, Action::Deselect).enabled(picked);
    if clicked(ui, &none) {
        shortcuts::act(window, Action::Deselect);
    }
    let pick_first = tr!("menu-pick-first");
    let why = (!picked).then_some(pick_first.as_str());
    let duplicate = tr!("action-duplicate");
    let duplicate = action_row(Some(icon::DUPLICATE), &duplicate, Action::Duplicate).unless(why);
    if clicked(ui, &duplicate) {
        crate::panels::duplicate_selection(window);
    }
    let remove = tr!("action-remove");
    let remove = action_row(Some(icon::REMOVE), &remove, Action::Remove).enabled(picked);
    if clicked(ui, &remove) {
        shortcuts::act(window, Action::Remove);
    }
    move_to_plate(ui, window.doc);
}

/// Laying the plate out, turning every model on it, and preparing it for print.
fn plate_items(ui: &mut egui::Ui, window: &mut Window) {
    let anything = window
        .doc
        .scene
        .has_printable(window.doc.scene.active_plate());
    let empty = tr!("menu-plate-empty");
    let why = (!anything).then_some(empty.as_str());
    let arrange = tr!("menu-arrange-plate");
    if clicked(ui, &MenuRow::new(Some(icon::ARRANGE), &arrange).unless(why)) {
        let gap_mm = window.tools.array.gap_mm;
        window.machine.status =
            crate::arrange::arrange_plate(&mut window.doc.scene, &window.doc.plate, gap_mm);
    }
    let can_orient = window.tools.orient.blocker(&window.doc.scene).is_none()
        && !window.tools.orient.is_running()
        && crate::prepare::holding(window.tools).is_none();
    let orient = tr!("menu-auto-orient-plate");
    let orient = MenuRow::new(Some(icon::AUTO_ORIENT), &orient).enabled(can_orient);
    if clicked(ui, &orient) {
        crate::panels::auto_orient(window, None);
    }
    let blocked = crate::prepare::blocker(window.doc, window.tools, window.machine);
    let prepare = tr!("menu-prepare");
    let prepare = MenuRow::new(Some(icon::PREPARE), &prepare).unless(blocked.as_deref());
    if clicked(ui, &prepare) {
        crate::panels::prepare_row::start(window);
    }
}

/// Sends everything selected to another plate, the one it stands on ticked. Off with
/// nothing selected, and with only one plate to send it to.
fn move_to_plate(ui: &mut egui::Ui, doc: &mut Doc) {
    let selected: Vec<ObjectId> = doc.scene.selection().to_vec();
    let plates: Vec<String> = doc.scene.plates().to_vec();
    let active = doc.scene.active_plate();
    let enabled = !selected.is_empty() && plates.len() > 1;

    let mut moved = None;
    let label = tr!("menu-move-to-plate");
    let row = MenuRow::new(Some(icon::MOVE_TO_PLATE), &label).enabled(enabled);
    menu_sub(ui, &row, |ui| {
        for (index, name) in plates.iter().enumerate() {
            let plate = index as u32;
            let here = plate == active;
            let row = MenuRow::new(None, name).ticked(here).enabled(!here);
            if menu_row(ui, &row).clicked() {
                moved = Some(plate);
                ui.close();
            }
        }
    });
    if let Some(plate) = moved {
        for id in selected {
            doc.scene.move_to_plate(id, plate);
        }
    }
}
