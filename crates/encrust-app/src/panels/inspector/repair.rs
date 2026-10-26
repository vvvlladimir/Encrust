use core_geometry::{MendMethod, MeshDiagnostics};
use core_volume::RepairMethod;

use crate::i18n::tr;
use crate::panels::Window;
use crate::repair::{Finding, Found, RepairState};
use crate::scene::{ObjectId, Scene, SceneObject};
use crate::ui::{
    blocked_hover, busy, describe, icon, issue_row, list, list_row, number_row, picker,
    primary_button, readings, section, section_with_action, theme, tone,
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
    if window.doc.repairs.found.shown().is_some() {
        window.doc.repairs.found.place(&window.doc.scene, &ids);
    }
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
        let objects: Vec<&SceneObject> = ids
            .iter()
            .filter_map(|id| window.doc.scene.get(*id))
            .collect();
        let crossings = crossings(&window.doc.repairs.found, &objects);
        let rows = faults(&objects, crossings);
        let shells = sum(&objects, |found| found.shells).to_string();
        let crossed = match crossings {
            Crossings::Counted(0) => Some("0".to_owned()),
            Crossings::Counted(_) => None,
            Crossings::Counting => Some(tr!("repair-counting")),
            Crossings::Uncounted => Some(tr!("repair-uncounted")),
        };
        marking(ui, &mut window.doc.repairs.found, &window.doc.scene, &rows);
        let mut readings_rows = vec![(tr!("repair-shells"), shells)];
        readings_rows.extend(crossed.map(|crossed| (tr!("repair-crossing-faces"), crossed)));
        readings(ui, &readings_rows);
    });
    if pressed {
        window
            .doc
            .repairs
            .found
            .count_crossings(&window.doc.scene, &ids);
    }
}

/// One row per kind of fault the models have, each press going to its next place and the
/// one after the last putting the marks away; a verdict where they have none.
fn marking(ui: &mut egui::Ui, found: &mut Found, scene: &Scene, rows: &[(String, usize, Finding)]) {
    if rows.is_empty() {
        tone(ui, &tr!("repair-nothing-found"), theme::colors().ok);
        return;
    }
    list(ui, |ui| {
        for (name, count, finding) in rows {
            let shown = found.shown() == Some(*finding);
            let tint = theme::scene().fault;
            if issue_row(ui, tint, name, &count.to_string(), shown).clicked() {
                found.press(scene, *finding);
            }
        }
    });
}

/// The kinds of fault the models have, each with its count and what marks it.
fn faults(objects: &[&SceneObject], crossings: Crossings) -> Vec<(String, usize, Finding)> {
    let crossing = match crossings {
        Crossings::Counted(pairs) => pairs,
        Crossings::Counting | Crossings::Uncounted => 0,
    };
    [
        (
            tr!("repair-holes"),
            sum(objects, |found| found.holes),
            Finding::OpenEdges,
        ),
        (
            tr!("repair-open-edges"),
            sum(objects, |found| found.boundary_edges),
            Finding::OpenEdges,
        ),
        (
            tr!("repair-flat-faces"),
            sum(objects, |found| found.degenerate_faces),
            Finding::FlatFaces,
        ),
        (
            tr!("repair-doubled-faces"),
            sum(objects, |found| found.duplicate_faces),
            Finding::DoubledFaces,
        ),
        (
            tr!("repair-crossing-faces"),
            crossing,
            Finding::CrossingFaces,
        ),
    ]
    .into_iter()
    .filter(|(_, count, _)| *count > 0)
    .collect()
}

fn sum(objects: &[&SceneObject], count: fn(&MeshDiagnostics) -> usize) -> usize {
    objects
        .iter()
        .map(|object| count(&object.summary.diagnostics))
        .sum()
}

/// How far counting the crossing faces of the models has got.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Crossings {
    /// Pairs over every model.
    Counted(usize),
    Counting,
    /// A model not counted as it stands, or whose count gave no answer.
    Uncounted,
}

fn crossings(found: &Found, objects: &[&SceneObject]) -> Crossings {
    let counts: Vec<_> = objects
        .iter()
        .map(|object| found.crossings_of(object))
        .collect();
    if counts.iter().any(Option::is_none) {
        return Crossings::Uncounted;
    }
    match counts.into_iter().flatten().sum::<Option<usize>>() {
        Some(total) => Crossings::Counted(total),
        None if found.is_working() => Crossings::Counting,
        None => Crossings::Uncounted,
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
