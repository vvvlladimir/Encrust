use crate::state::Tools;
use crate::ui::{describe, icon, secondary_button, section, stats};

/// What the two picked points are apart, along each axis and in a straight line.
pub fn ui(ui: &mut egui::Ui, tools: &mut Tools) {
    section(ui, "Measure", None, |ui| {
        let Some(delta) = tools.measure.delta_mm() else {
            describe(
                ui,
                match tools.measure.start() {
                    Some(_) => "Click the second point.",
                    None => "Click two points on a model. Alt-click clears.",
                },
            );
            return;
        };

        let distance_mm = tools.measure.distance_mm().unwrap_or_default();
        stats(
            ui,
            &[
                ("Distance", format!("{distance_mm:.2} mm")),
                ("dX", format!("{:.2} mm", delta.x)),
                ("dY", format!("{:.2} mm", delta.y)),
                ("dZ", format!("{:.2} mm", delta.z)),
            ],
        );
        ui.add_space(6.0);
        if secondary_button(ui, icon::CANCEL, "Clear").clicked() {
            tools.measure.clear();
        }
    });
}
