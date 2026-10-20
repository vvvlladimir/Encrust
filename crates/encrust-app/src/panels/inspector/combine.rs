use core_volume::Boolean;

use crate::i18n::tr;
use crate::panels::Window;
use crate::ui::{
    Segment, Segmented, blocked_hover, busy, describe, icon, primary_button, section, stats, theme,
    tone,
};

/// What the picked models are to become, and which of them they are.
pub fn sections(ui: &mut egui::Ui, window: &mut Window) {
    let tool = &mut window.tools.boolean;
    section(ui, &tr!("boolean-operation"), None, |ui| {
        let segments = [
            Segment::new(Boolean::Union, tr!("boolean-union")),
            Segment::new(Boolean::Difference, tr!("boolean-difference")),
            Segment::new(Boolean::Intersection, tr!("boolean-intersection")),
        ];
        Segmented::new(&segments)
            .width(ui.available_width())
            .show(ui, &mut tool.op);
        describe(
            ui,
            &match tool.op {
                Boolean::Union => tr!("boolean-union-note"),
                Boolean::Difference => tr!("boolean-difference-note"),
                Boolean::Intersection => tr!("boolean-intersection-note"),
            },
        );
    });
    let op = tool.op;
    section(ui, &tr!("boolean-models"), None, |ui| {
        let picked = crate::boolean::picked_names(&window.doc.scene);
        if picked.is_empty() {
            tone(ui, &tr!("boolean-none-picked"), theme::colors().text_low);
            return;
        }
        let rows: Vec<(String, String)> = picked
            .into_iter()
            .enumerate()
            .map(|(index, name)| (name, role(op, index)))
            .collect();
        stats(ui, &rows);
        describe(ui, &tr!("boolean-models-note"));
    });
}

/// What one picked model does in the operation, by its place in the order they were picked.
fn role(op: Boolean, index: usize) -> String {
    match (op, index) {
        (Boolean::Difference, 0) => tr!("boolean-role-kept"),
        (Boolean::Difference, _) => tr!("boolean-role-taken-out"),
        (Boolean::Union, _) => tr!("boolean-role-joined"),
        (Boolean::Intersection, _) => tr!("boolean-role-shared"),
    }
}

/// Combining the picked models, or the progress of the run that is.
pub fn action(ui: &mut egui::Ui, window: &mut Window) {
    let tool = &mut window.tools.boolean;
    if tool.is_busy() {
        let step = tool
            .counted()
            .map(|counted| (counted.label(), counted.share));
        let step = step.as_ref().map(|(label, share)| (label.as_str(), *share));
        if busy(ui, &tr!("boolean-combining"), step, true) {
            tool.cancel(&mut window.machine.status);
        }
        return;
    }
    let blocked = tool.blocker(&window.doc.scene);
    let count = crate::boolean::picked_names(&window.doc.scene).len();
    let label = match count {
        0 | 1 => tr!("boolean-combine-none"),
        count => tr!("boolean-combine", count = count),
    };
    let button = primary_button(ui, icon::COMBINE, &label, blocked.is_none());
    if blocked_hover(button, blocked).clicked() {
        let started = tool.start(&window.doc.scene);
        window
            .machine
            .status
            .report(&tr!("boolean-combining"), started);
    }
}
