use core_analysis::{Measured, equivalent_disc_mm};
use core_format::{ExposurePlan, PrintJob};

use crate::panels::Window;
use crate::state::Machine;
use crate::ui::{duration, hint, readings, section, stats};

/// What the print takes, and what the layer the section rail is parked on costs to print.
/// The mask itself stands beside the model on the stage rather than in this column.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    if window.machine.preview.layer_count() == 0 {
        section(ui, "This layer", None, |ui| {
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
        section(ui, "This layer", None, |ui| {
            stats(ui, &layer_readings(window));
        });
        return;
    }
    print(ui, window);
    super::issues::entry(ui, window);
    pull(ui, window);

    let layer = window.machine.preview.layer();
    section(ui, "This layer", Some(block(window.machine, layer)), |ui| {
        stats(ui, &layer_readings(window));
    });
    super::print_settings::ui(ui, window.machine);
}

/// The machine, the resin, and what the stack comes to on them, measured from the masks.
fn print(ui: &mut egui::Ui, window: &mut Window) {
    let Some(printer) = window.machine.slicing.printer.as_ref() else {
        return;
    };
    let title = printer.name.clone();
    let rows = print_readings(window);
    section(ui, &title, None, |ui| readings(ui, &rows));
}

fn print_readings(window: &Window) -> Vec<(&'static str, String)> {
    let material = &window.machine.slicing.material;
    let mut rows = vec![("Resin", material.name.clone())];
    match window.measured() {
        Some(measured) => {
            let volume_mm3 = measured.volume_mm3();
            let weight_g = volume_mm3 / 1000.0 * material.density_g_cm3;
            let price = material.cost(weight_g, volume_mm3).map_or_else(
                || "not priced".to_owned(),
                |cost| format!("{cost:.3} {}", material.details.currency),
            );
            rows.extend([
                ("Volume", format!("{:.3} ml", volume_mm3 / 1000.0)),
                ("Weight", format!("{weight_g:.3} g")),
                ("Price", price),
            ]);
        }
        None => {
            let waiting = if window.machine.preview.is_measuring() {
                "measuring"
            } else {
                "-"
            };
            rows.extend(["Volume", "Weight", "Price"].map(|key| (key, waiting.to_owned())));
        }
    }
    if let Some(seconds) = print_time_s(window.machine) {
        rows.push(("Time", duration(seconds)));
    }
    rows
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

/// The stack as the writer would see it, which is what every per-layer reading here is
/// taken from. `None` until a printer is chosen and the stack is planned.
fn job(machine: &Machine) -> Option<PrintJob> {
    let slicing = &machine.slicing;
    Some(PrintJob {
        printer: slicing.printer.clone()?,
        material: slicing.material.clone(),
        raster: slicing.raster_settings()?,
        plan: machine.preview.plan()?.clone(),
        volume_mm3: 0.0,
        exposure: ExposurePlan::new(slicing.exposure.clone()),
        thumbnail: None,
        created_unix_s: 0,
    })
}

/// What the machine takes over the stack: every layer's exposure, waits and travel.
fn print_time_s(machine: &Machine) -> Option<u32> {
    Some(job(machine)?.print_time_s())
}

/// Where `layer` sits in the resin's exposure ramp, which is the heading this section
/// carries.
fn block(machine: &Machine, layer: usize) -> &'static str {
    let material = &machine.slicing.material;
    let bottom = material.bottom_layers as usize;
    match layer {
        layer if layer < bottom => "bottom block",
        layer if layer < bottom + material.transition_layers as usize => "transition",
        _ => "body",
    }
}

/// Exposure of `layer`, seconds: the value the writer would give it, and the resin's own
/// ramp before a printer is chosen.
fn layer_exposure_s(machine: &Machine, layer: usize) -> f32 {
    match job(machine) {
        Some(job) => job.exposure_of_layer_s(layer as u32),
        None => machine.slicing.material.exposure_of_layer_s(layer as u32),
    }
}

fn layer_readings(window: &Window) -> Vec<(&'static str, String)> {
    let preview = &window.machine.preview;
    // A file's own table, where one is open: the window's resin says nothing about a
    // stack somebody else exposed.
    let exposure_s = preview
        .read_layer_exposure_s()
        .unwrap_or_else(|| layer_exposure_s(window.machine, preview.layer()));

    vec![
        ("Layer", format!("{}", preview.layer() + 1)),
        (
            "Z height",
            format!("{:.3} mm", preview.layer_z().unwrap_or_default()),
        ),
        ("Exposure", format!("{exposure_s:.2} s")),
        (
            "Cured area",
            format!("{:.0} mm2", preview.layer_area_mm2().unwrap_or_default()),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn machine_with_a_ramp() -> Machine {
        let mut machine = Machine::default();
        let material = &mut machine.slicing.material;
        material.bottom_layers = 5;
        material.transition_layers = 5;
        material.bottom_exposure_s = 30.0;
        material.exposure_s = 5.2;
        machine
    }

    #[test]
    fn a_transition_layer_reads_its_own_step_of_the_ramp() {
        let machine = machine_with_a_ramp();
        // Six equal steps from 30 s to 5.2 s, so the first transition layer is one down.
        let first = layer_exposure_s(&machine, 5);
        assert!(
            (first - (30.0 - 24.8 / 6.0)).abs() < 1e-3,
            "expected 25.867 s on the first transition layer, got {first}"
        );
        assert_eq!(
            layer_exposure_s(&machine, 4),
            30.0,
            "still the bottom block"
        );
        assert_eq!(layer_exposure_s(&machine, 10), 5.2, "past the ramp");
    }

    #[test]
    fn the_heading_names_the_block_the_layer_is_in() {
        let machine = machine_with_a_ramp();
        assert_eq!(block(&machine, 4), "bottom block");
        assert_eq!(block(&machine, 5), "transition");
        assert_eq!(block(&machine, 10), "body");
    }
}
