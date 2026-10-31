//! What each action does to the window.

use crate::i18n::tr;
use crate::panels::{
    Window, duplicate_selection, frame_view, section, slice_this_plate, toggle_settings,
};
use crate::status::Status;
use crate::workspace::Tool;

use super::Action;

/// Esc over the Machine and resin window: the question or the calculator over it first,
/// then the window itself.
fn leave_machines(machine: &mut crate::state::Machine) {
    let settings = &mut machine.settings;
    if std::mem::take(&mut settings.reset_unreadable) || settings.calculators.open.take().is_some()
    {
        return;
    }
    settings.close_machines();
}

fn leave_settings(machine: &mut crate::state::Machine) {
    if !std::mem::take(&mut machine.settings.reset_unreadable) {
        machine.settings.close();
    }
}

/// The printer's sheet over the update sheet, as they are drawn: modal, and Esc has to reach
/// them here for the same reason as the report's.
fn held_by_a_sheet(machine: &mut crate::state::Machine, action: Action) -> bool {
    let leave = matches!(action, Action::Deselect);
    if machine.network.open.is_some() {
        if leave {
            machine.network.open = None;
        }
        return true;
    }
    if machine.updates.sheet {
        if leave {
            machine.updates.sheet = false;
        }
        return true;
    }
    false
}

/// Answers `action` for the modal window in view, if one is, which holds the keyboard
/// until it closes. Answers whether one was.
fn held_by_a_modal(window: &mut Window, action: Action) -> bool {
    // The sheet is a modal: it holds the keyboard until it is closed.
    if window.view.options.sheet {
        if matches!(action, Action::Sheet | Action::Deselect) {
            window.view.options.sheet = false;
            super::keymap::record(None);
        }
        return true;
    }
    // The report sheet is modal as well, and Esc has to reach it here: egui never sees
    // the key, because this is read before its pass.
    if window.machine.report.open {
        if matches!(action, Action::Deselect) {
            window.machine.report.open = false;
        }
        return true;
    }
    if held_by_a_sheet(window.machine, action) {
        return true;
    }
    // The Machine and resin window is modal as well: it holds the keyboard, Esc takes
    // down whatever stands over it first, and undo reaches the form in view.
    if window.machine.settings.machines {
        match action {
            Action::Deselect => leave_machines(window.machine),
            Action::Undo => _ = window.machine.settings.undo(),
            Action::Redo => _ = window.machine.settings.redo(),
            _ => {}
        }
        return true;
    }
    // So is the Settings window, and Esc takes down the question over it first.
    if window.machine.settings.open {
        match action {
            Action::Deselect => leave_settings(window.machine),
            Action::Undo => _ = window.machine.settings.undo(),
            Action::Redo => _ = window.machine.settings.redo(),
            _ => {}
        }
        return true;
    }
    false
}

/// Does what a key asked for. Every action of the table is answered here, so a binding
/// cannot be added without the work behind it.
pub fn act(window: &mut Window, action: Action) {
    if held_by_a_modal(window, action) {
        return;
    }
    match action {
        Action::Pick(tool) => pick(window, tool),
        Action::Undo => {
            window.doc.history.undo(
                &mut window.doc.scene,
                window.tools,
                &mut window.machine.slicing,
            );
        }
        Action::Redo => {
            window.doc.history.redo(
                &mut window.doc.scene,
                window.tools,
                &mut window.machine.slicing,
            );
        }
        Action::SelectAll => window.doc.scene.select_here(),
        // A measurement in hand goes first, so Esc on the ruler does not drop the selection.
        Action::Deselect if *window.tool == Tool::Measure && !window.tools.measure.is_empty() => {
            window.tools.measure.clear();
        }
        Action::Deselect => {
            window.tools.orient.stop_picking();
            window.tools.cut.picking_part = false;
            window.doc.scene.clear_selection();
        }
        Action::Duplicate => duplicate_selection(window),
        Action::Remove => remove(window),
        Action::View(step) => section::turn_view(window, step),
        Action::FrameView => frame_view(
            &window.doc.scene,
            &window.doc.plate,
            &mut window.view.camera,
        ),
        Action::Settings => toggle_settings(window.machine),
        Action::Sheet => window.view.options.sheet = true,
        Action::Find => window.view.options.find = true,
        Action::Step(layers) => section::step(window, layers),
        Action::Play => section::play(window),
        Action::NewPlate => crate::project::new_plate(window),
        Action::NewProject => crate::project::new_project(window),
        Action::OpenProject => crate::project::open_dialog(window),
        Action::SaveProject => {
            crate::project::save_open(window);
        }
        Action::SaveProjectAs => {
            crate::project::save_dialog(window);
        }
        Action::OpenModel => crate::import::open_dialog(window),
        Action::Slice => slice_this_plate(window),
    }
}

/// Opens a tool in the inspector, or folds the inspector away when the tool is open
/// already. The view stays as it is: the layer strip alone switches it; see
/// `docs/decisions/0218`.
pub fn pick(window: &mut Window, tool: Tool) {
    let options = &mut window.view.options;
    options.inspector = *window.tool != tool || !options.inspector;
    *window.tool = tool;
}

fn remove(window: &mut Window) {
    let gone = window.doc.scene.remove_selected();
    if gone > 0 {
        window.machine.status = Status::Info(tr!("status-removed-models", count = gone));
    }
}
