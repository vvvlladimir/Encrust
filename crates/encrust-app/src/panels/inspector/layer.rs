use core_analysis::{Measured, equivalent_disc_mm};

use crate::panels::Window;
use crate::ui::{hint, readings, section};

/// What the stack is printed with and what pulls on it. The layer's own figures stand at
/// the foot of the plate column, and the mask beside the model on the stage.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    if window.machine.preview.layer_count() == 0 {
        section(ui, "Layers", None, |ui| {
            hint(ui, "No stack to show yet.");
        });
        return;
    }

    if window.view.options.issues {
        super::issues::ui(ui, window);
        return;
    }

    // A file being shown states its own numbers; ours describe a plate it knows nothing of.
    if window.machine.preview.read_facts().is_some() {
        super::opened::ui(ui, window);
        return;
    }
    super::issues::entry(ui, window);
    pull(ui, window);
    super::print_settings::ui(ui, window.machine);
}

/// Which layers pull hardest on the film as the plate lifts, above the bottom block.
///
/// Every row is listed whether its peak is known or not: a print setting edited here is
/// measured again, and a block that comes and goes takes the field under it out from
/// under the pointer.
fn pull(ui: &mut egui::Ui, window: &mut Window) {
    let measured = window.measured();
    if measured.is_none() && !window.machine.preview.is_measuring() {
        return;
    }
    let waiting = || "measuring".to_owned();
    let hardest = measured.and_then(Measured::hardest_pull);
    let widest = measured.and_then(Measured::largest_growth);
    let rows = vec![
        (
            "Hardest pull",
            hardest.map_or_else(waiting, |peak| format!("layer {}", peak.layer + 1)),
        ),
        (
            "Pulls like a disc of",
            hardest.map_or_else(waiting, |peak| {
                format!("{:.1} mm", equivalent_disc_mm(peak.value))
            }),
        ),
        (
            "Widest step",
            widest.map_or_else(waiting, |peak| format!("layer {}", peak.layer + 1)),
        ),
        (
            "Grows by",
            widest.map_or_else(waiting, |peak| format!("{:.0} mm2", peak.value)),
        ),
    ];
    section(ui, "Peel", None, |ui| readings(ui, &rows));
}
