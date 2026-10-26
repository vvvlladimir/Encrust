use core_geometry::{MendMethod, MeshDiagnostics};
use core_volume::RepairMethod;

use crate::i18n::tr;
use crate::panels::Window;
use crate::repair::RepairState;
use crate::scene::ObjectId;
use crate::ui::{
    blocked_hover, busy, describe, icon, list, list_row, number_row, picker, primary_button,
    readings, section, section_with_action, theme, tone,
};

/// Every repair the Method picker offers, in the order it lists them.
const METHODS: [RepairMethod; 6] = [
    RepairMethod::Mend(MendMethod::Repair),
    RepairMethod::Mend(MendMethod::CloseHoles),
    RepairMethod::Mend(MendMethod::CloseSmallHoles),
    RepairMethod::Mend(MendMethod::Turn),
    RepairMethod::Wrap,
    RepairMethod::Deep,
];

/// Per point of drag, mm or mm².
const STEP: f64 = 0.01;

/// Bounds on the lengths the panel edits: a seam wider than a millimetre is a gap, a speck
/// past a square centimetre a part, and a lattice under 0.05 mm more than memory holds.
const MAX_SEAM_MM: f32 = 1.0;
const MAX_SHELL_MM2: f32 = 100.0;
const WRAP_MM: std::ops::RangeInclusive<f32> = 0.05..=2.0;

// TODO(step-9): the Shells section, listing each shell with its faces and dropping those
// picked, lands with `--drop-shells` so the command line can do it too.

/// What is wrong with the models the tool works on, how to repair them, and the lengths
/// the repair works to.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    found(ui, window);
    method(ui, &mut window.tools.repair);
    settings(ui, &mut window.tools.repair);
}

/// What import found in the models, summed, with the crossing faces counted on demand.
fn found(ui: &mut egui::Ui, window: &mut Window) {
    let ids: Vec<ObjectId> = window.doc.scene.targets().map(|object| object.id).collect();
    let action = (icon::FIND, tr!("repair-count-crossings"));
    let (pressed, _) = section_with_action(ui, &tr!("repair-found"), (action.0, &action.1), |ui| {
        describe(ui, &tr!("repair-found-note"));
        if ids.is_empty() {
            tone(
                ui,
                &tr!("repair-nothing-on-plate"),
                theme::colors().text_low,
            );
            return;
        }
        readings(ui, &rows(window, &ids));
    });
    if pressed {
        window.doc.repairs.count_crossings(&window.doc.scene, &ids);
    }
}

/// One reading per kind of fault, summed over the models in `ids`.
fn rows(window: &Window, ids: &[ObjectId]) -> Vec<(String, String)> {
    let scene = &window.doc.scene;
    let objects: Vec<_> = ids.iter().filter_map(|id| scene.get(*id)).collect();
    let sum = |count: fn(&MeshDiagnostics) -> usize| -> String {
        let total: usize = objects
            .iter()
            .map(|object| count(&object.summary.diagnostics))
            .sum();
        total.to_string()
    };
    vec![
        (tr!("repair-holes"), sum(|found| found.holes)),
        (tr!("repair-open-edges"), sum(|found| found.boundary_edges)),
        (
            tr!("repair-flat-faces"),
            sum(|found| found.degenerate_faces),
        ),
        (
            tr!("repair-doubled-faces"),
            sum(|found| found.duplicate_faces),
        ),
        (tr!("repair-shells"), sum(|found| found.shells)),
        (tr!("repair-crossing-faces"), crossings(window, &objects)),
    ]
}

/// The crossing pairs summed, a dash until every model is counted, an ellipsis while one is.
fn crossings(window: &Window, objects: &[&crate::scene::SceneObject]) -> String {
    let counts: Vec<_> = objects
        .iter()
        .map(|object| window.doc.repairs.crossings_of(object))
        .collect();
    if counts.iter().any(Option::is_none) {
        return tr!("repair-uncounted");
    }
    match counts.into_iter().flatten().sum::<Option<usize>>() {
        Some(total) => total.to_string(),
        None if window.doc.repairs.is_counting() => tr!("repair-counting"),
        None => tr!("repair-uncounted"),
    }
}

/// The repair the button runs, chosen from a list that drops from the picker.
fn method(ui: &mut egui::Ui, state: &mut RepairState) {
    section(ui, &tr!("repair-method"), None, |ui| {
        describe(ui, &note(state.method));
        let response = picker(ui, icon::REPAIR, &name(state.method));
        egui::Popup::menu(&response)
            .width(response.rect.width())
            .show(|ui| {
                list(ui, |ui| {
                    for method in METHODS {
                        let chosen = method == state.method;
                        let row = list_row(ui, &name(method), None, chosen, None, None, None, None);
                        if row.picked {
                            state.method = method;
                            ui.close();
                        }
                    }
                });
            });
    });
}

/// The lengths the chosen method works to; a method that measures nothing has no section.
fn settings(ui: &mut egui::Ui, state: &mut RepairState) {
    let mends = matches!(
        state.method,
        RepairMethod::Mend(MendMethod::Repair) | RepairMethod::Wrap | RepairMethod::Deep
    );
    let wraps = state.method.wraps();
    if !mends {
        return;
    }
    section(ui, &tr!("repair-settings"), None, |ui| {
        describe(ui, &tr!("repair-settings-note"));
        let settings = &mut state.settings;
        number_row(
            ui,
            &tr!("repair-seam-tolerance"),
            &mut settings.mend.seam_tolerance_mm,
            &tr!("unit-mm"),
            STEP,
            0.0..=MAX_SEAM_MM,
            2,
        );
        number_row(
            ui,
            &tr!("repair-smallest-shell"),
            &mut settings.mend.smallest_shell_mm2,
            &tr!("unit-mm2"),
            STEP,
            0.0..=MAX_SHELL_MM2,
            2,
        );
        if wraps {
            number_row(
                ui,
                &tr!("repair-wrap-lattice"),
                &mut settings.wrap_mm,
                &tr!("unit-mm"),
                STEP,
                WRAP_MM,
                2,
            );
        }
    });
}

/// Repairing the models the tool works on, or the progress of the repairs that are.
pub fn action(ui: &mut egui::Ui, window: &mut Window) {
    let repairs = &mut window.doc.repairs;
    if repairs.is_busy() {
        let step = repairs
            .counted()
            .map(|counted| (counted.label(), counted.share));
        let step = step.as_ref().map(|(label, share)| (label.as_str(), *share));
        if busy(ui, &tr!("repair-repairing"), step, true) {
            repairs.cancel(&mut window.machine.status);
        }
        return;
    }
    let ids: Vec<ObjectId> = window.doc.scene.targets().map(|object| object.id).collect();
    let blocked = ids.is_empty().then(|| tr!("repair-nothing-on-plate"));
    let button = primary_button(ui, icon::REPAIR, &tr!("repair-repair"), blocked.is_none());
    if blocked_hover(button, blocked).clicked() {
        let state = window.tools.repair;
        for id in ids {
            repairs.start(&window.doc.scene, id, state, &mut window.machine.status);
        }
    }
}

/// How many of the models the tool works on will not slice as they stand, in the colour
/// that says so; nothing while there is no model.
pub fn fact(window: &Window) -> Option<(String, egui::Color32)> {
    let colors = theme::colors();
    let mut targets = window.doc.scene.targets().peekable();
    targets.peek()?;
    match targets.filter(|object| !object.summary.is_sound()).count() {
        0 => Some((tr!("repair-sound"), colors.ok)),
        broken => Some((tr!("repair-broken", count = broken), colors.warn)),
    }
}

fn name(method: RepairMethod) -> String {
    match method {
        RepairMethod::Mend(MendMethod::Repair) => tr!("repair-method-repair"),
        RepairMethod::Mend(MendMethod::CloseHoles) => tr!("repair-method-close-holes"),
        RepairMethod::Mend(MendMethod::CloseSmallHoles) => tr!("repair-method-close-small-holes"),
        RepairMethod::Mend(MendMethod::Turn) => tr!("repair-method-turn"),
        RepairMethod::Wrap => tr!("repair-method-wrap"),
        RepairMethod::Deep => tr!("repair-method-deep"),
    }
}

fn note(method: RepairMethod) -> String {
    match method {
        RepairMethod::Mend(MendMethod::Repair) => tr!("repair-method-repair-note"),
        RepairMethod::Mend(MendMethod::CloseHoles) => tr!("repair-method-close-holes-note"),
        RepairMethod::Mend(MendMethod::CloseSmallHoles) => {
            tr!("repair-method-close-small-holes-note")
        }
        RepairMethod::Mend(MendMethod::Turn) => tr!("repair-method-turn-note"),
        RepairMethod::Wrap => tr!("repair-method-wrap-note"),
        RepairMethod::Deep => tr!("repair-method-deep-note"),
    }
}
