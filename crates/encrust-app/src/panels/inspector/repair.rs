use core_geometry::{MendMethod, MeshDiagnostics};
use core_volume::RepairMethod;

use crate::i18n::tr;
use crate::job::Counted;
use crate::panels::Window;
use crate::repair::{Finding, Found, RepairMode, RepairState};
use crate::scene::{ObjectId, Scene, SceneObject};
use crate::ui::{
    DropdownFace, DropdownRow, blocked_hover, describe, dropdown, dropdown_menu, dropdown_row,
    icon, icon_toggle, issue_list, issue_row, number_row, primary_button, readings, section,
    section_with_action, step_bar, theme, tone,
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

/// Bounds on the lengths the panel edits: a seam wider than a millimetre is a gap, and a
/// speck past a square centimetre a part.
const MAX_SEAM_MM: f32 = 1.0;
const MAX_SHELL_MM2: f32 = 100.0;

mod faces;
mod shells;

/// What is wrong with the models the tool works on, how to repair them, and the lengths
/// the repair works to; or, in the Faces mode, what is picked to change by hand.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    if window.tools.repair.mode == RepairMode::Faces {
        faces::ui(ui, window);
        return;
    }
    found(ui, window);
    method(ui, &mut window.tools.repair.state);
    settings(ui, &mut window.tools.repair.state);
    shells::listed(ui, window);
}

/// The card's two modes, in its heading.
pub fn modes(ui: &mut egui::Ui, window: &mut Window) {
    let modes = [
        (RepairMode::Faces, icon::PAINT, tr!("repair-mode-faces")),
        (RepairMode::Model, icon::MODEL, tr!("repair-mode-model")),
    ];
    ui.spacing_mut().item_spacing.x = theme::ICON_GAP;
    let tool = &mut window.tools.repair;
    for (mode, glyph, name) in modes {
        if icon_toggle(ui, glyph, &name, tool.mode == mode).clicked() {
            tool.mode = mode;
        }
    }
}

/// What import found in the models, summed, with the crossing faces counted on demand.
fn found(ui: &mut egui::Ui, window: &mut Window) {
    let ids: Vec<ObjectId> = window.doc.scene.targets().map(|object| object.id).collect();
    if window.doc.repairs.found.shown().is_some() {
        window.doc.repairs.found.place(&window.doc.scene, &ids);
    }
    let action = (icon::RESCAN, tr!("repair-count-crossings"));
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
        let mut readings_rows = Vec::new();
        if sum(&objects, |found| found.holes) > 0 {
            let open = sum(&objects, |found| found.boundary_edges).to_string();
            readings_rows.push((tr!("repair-open-edges"), open));
        }
        readings_rows.push((tr!("repair-shells"), shells));
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
    issue_list(ui, |ui| {
        for (name, count, finding) in rows {
            let shown = found.shown() == Some(*finding);
            let tint = theme::scene().fault;
            if issue_row(ui, tint, (name, None), &count.to_string(), shown).clicked() {
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
    // Holes are loops of the open edges, so one row marks both; the edges count is a
    // reading then, and a row only where they close no loop.
    let holes = sum(objects, |found| found.holes);
    let (open, count) = match holes {
        0 => (
            tr!("repair-open-edges"),
            sum(objects, |found| found.boundary_edges),
        ),
        _ => (tr!("repair-holes"), holes),
    };
    [
        (open, count, Finding::OpenEdges),
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
        let shown = name(state.method);
        let face = DropdownFace::plain(icon::REPAIR, &shown);
        let response = dropdown(ui, &face, ui.available_width());
        dropdown_menu(&response, |ui| {
            for method in METHODS {
                let named = name(method);
                let row = DropdownRow::named(&named, method == state.method);
                if dropdown_row(ui, &row).clicked() {
                    state.method = method;
                    ui.close();
                }
            }
        });
    });
}

/// The lengths the chosen method works to; a method that measures nothing has no section.
fn settings(ui: &mut egui::Ui, state: &mut RepairState) {
    let mends = matches!(
        state.method,
        RepairMethod::Mend(MendMethod::Repair) | RepairMethod::Wrap | RepairMethod::Deep
    );
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
    });
}

/// Repairing the models the tool works on, or the progress of the repairs that are; in
/// the Faces mode, the change by hand.
pub fn action(ui: &mut egui::Ui, window: &mut Window) {
    if window.tools.repair.mode == RepairMode::Faces {
        faces::action(ui, window);
        return;
    }
    shells::drop_picked(ui, window);
    let repairs = &mut window.doc.repairs;
    if repairs.is_busy() {
        let counted = repairs.counted();
        let what = counted
            .as_ref()
            .map_or_else(|| tr!("repair-repairing"), Counted::label);
        if step_bar(ui, &what, counted.map(|counted| counted.share), true) {
            repairs.cancel(&mut window.machine.status);
        }
        return;
    }
    let ids: Vec<ObjectId> = window.doc.scene.targets().map(|object| object.id).collect();
    let blocked = ids.is_empty().then(|| tr!("repair-nothing-on-plate"));
    let button = primary_button(ui, &tr!("repair-repair"), blocked.is_none());
    if blocked_hover(button, blocked).clicked() {
        let state = window.tools.repair.state;
        for id in ids {
            repairs.start(&window.doc.scene, id, state, &mut window.machine.status);
        }
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
