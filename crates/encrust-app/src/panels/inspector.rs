mod cut;
mod drain;
mod hollow;
mod issues;
mod layer;
mod measure;
mod opened;
mod print_settings;
mod relief;
mod slicing;
mod supports;
mod transform;

pub use issues::tint as risk_tint;

use crate::panels::Window;
use crate::state::Doc;
use crate::ui::{describe, icon, secondary_button, section};
use crate::workspace::{Mode, Tool};

/// The column beside the rail: the tool in use, and nothing else. What is not about the
/// open tool is in the plate column; see `docs/decisions/0105`.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            match *window.mode {
                Mode::Prepare => prepare(ui, window),
                Mode::Preview => layer::ui(ui, window),
            }
        });
}

fn prepare(ui: &mut egui::Ui, window: &mut Window) {
    match *window.tool {
        Tool::Supports => supports::ui(ui, window),
        Tool::Hollow => hollow::ui(ui, window),
        Tool::Drain => drain::ui(ui, window),
        Tool::Cut => cut::ui(ui, window),
        Tool::Relief => relief::ui(ui, window),
        Tool::Layers => slicing::ui(ui, window.machine),
        Tool::Measure => measure::ui(ui, window.tools),
        _ if window.doc.scene.has_selection() => transform::ui(ui, window),
        _ => selection(ui, window.doc),
    }
}

/// The transform panel has nothing to stand on until something is picked, so the tool
/// says what picking does and offers the two selections that need no aim.
fn selection(ui: &mut egui::Ui, doc: &mut Doc) {
    let count = doc.scene.here().count();
    section(
        ui,
        "Selection",
        Some(&format!("{count} on the plate")),
        |ui| {
            describe(
                ui,
                "Click a model to pick it. Cmd-click adds one to the selection and \
                 shift-click takes everything between, and every tool then works on what \
                 is picked.",
            );
            ui.add_space(8.0);
            if secondary_button(ui, icon::SELECT, "Select all").clicked() {
                doc.scene.select_here();
            }
        },
    );
}
