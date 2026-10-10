use egui::{Color32, RichText};

use super::opened;
use crate::network::{Reach, Target};
use crate::panels::Window;
use crate::panels::plate_summary::file_readings;
use crate::panels::slice::{Run, Via, bound_machine, broken_reminder, reach_dot, start};
use crate::ui::{
    hint, icon, later, list, primary_button, readings, secondary_button, section,
    section_with_action, switch, text_row, theme,
};
use crate::workspace::Tool;

/// From the plate to a file, and from the file to a printer, top to bottom. Slice in the
/// plate column does the same press; this is where its choices are read.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    // A file being shown states its own numbers; ours describe a plate it knows nothing of.
    if window.machine.preview.read_facts().is_some() {
        opened::ui(ui, window);
    } else {
        file(ui, window);
    }
    send(ui, window);
}

/// Whether there is a file waiting on a machine, for the inspector's heading.
pub fn fact(window: &Window) -> Option<(String, Color32)> {
    let colors = theme::colors();
    if let Some(sent) = &window.machine.network.sent {
        return Some((format!("on {}", sent.printer), colors.accent_soft));
    }
    Some((
        format!(".{}", window.machine.slicing.format.extension()),
        colors.text_mid,
    ))
}

/// Starting the file that reached the machine; otherwise slicing, to the bound machine
/// where there is one and to a file where there is not.
pub fn action(ui: &mut egui::Ui, window: &mut Window) {
    if window.machine.network.sent.is_some() {
        if primary_button(ui, icon::PLAY, "Start the print", true).clicked() {
            window
                .machine
                .network
                .start_print(&mut window.machine.status);
        }
        hint(
            ui,
            "The file is on the machine. Nobody is standing at it yet.",
        );
        return;
    }

    let blocker = window.machine.slicing.blocker(&window.doc.scene);
    if blocker.is_none() {
        broken_reminder(ui, &window.doc.scene);
    }
    let busy = window.machine.slicing.job.is_some() || window.machine.network.job.is_some();
    let free = blocker.is_none() && !busy;
    let bound = bound_machine(window.machine);
    let to_file = format!("Slice to .{}", window.machine.slicing.format.extension());

    let mut pressed = None;
    match &bound {
        Some(bound) => {
            let label = format!("Slice and send to {}", bound.name);
            let enabled = free && bound.blocker.is_none();
            if primary_button(ui, icon::SEND, &label, enabled).clicked() {
                pressed = Some(Via::Printer);
            }
            if ui
                .add_enabled_ui(free, |ui| secondary_button(ui, icon::SAVE, &to_file))
                .inner
                .clicked()
            {
                pressed = Some(Via::File);
            }
            if let Some(why) = bound.blocker {
                hint(ui, why);
            }
        }
        None => {
            if primary_button(ui, icon::SLICE, &to_file, free).clicked() {
                pressed = Some(Via::File);
            }
        }
    }
    if let Some(why) = blocker {
        hint(ui, why);
    }
    if let Some(via) = pressed {
        start(
            &window.doc.scene,
            &mut window.machine.slicing,
            &mut window.machine.status,
            Run::ThisPlate,
            via,
        );
    }
}

/// What the file comes to, what it is written as, and the way to how it is cut.
fn file(ui: &mut egui::Ui, window: &mut Window) {
    let rows = file_readings(ui.ctx(), window);
    let format = crate::job::label_of(window.machine.slicing.format);
    let mut to_print_settings = false;
    section(ui, "The file", None, |ui| {
        readings(ui, &rows);
        // TODO(step-8): name the file, slice the selection or every plate, pick the
        // container, and leave the preview image out, all from here.
        later(ui, |ui| {
            let mut name = String::new();
            text_row(ui, "File name", &mut name);
            picker_row(ui, "export-scope", "Slice", "Whole plate");
            picker_row(ui, "export-format", "Format", format);
            let mut thumbnail = true;
            switch(ui, &mut thumbnail, "Preview image in the file");
        });
        to_print_settings =
            secondary_button(ui, icon::PARAMETERS, "Layers, exposure and edges").clicked();
    });
    if to_print_settings {
        crate::shortcuts::pick(window, Tool::PrintSettings);
    }
}

/// A drop-down that names its choice, under a label.
fn picker_row(ui: &mut egui::Ui, id: &str, label: &str, chosen: &str) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(label)
                .font(theme::label())
                .color(theme::colors().text_mid),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            egui::ComboBox::from_id_salt(id)
                .width(theme::FIELD_W)
                .selected_text(chosen)
                .show_ui(ui, |_| {});
        });
    });
}

/// Every machine the window can reach, with whether it answered, and the one the plate's
/// printer is bound to lit.
fn send(ui: &mut egui::Ui, window: &mut Window) {
    if cfg!(target_arch = "wasm32") {
        return;
    }
    let (rescan, _) =
        section_with_action(ui, "Send to a printer", (icon::RESET, "Look again"), |ui| {
            let network = &window.machine.network;
            let bound = network.target().map(|target| target.key());
            let targets = network.targets();
            if targets.is_empty() {
                hint(
                    ui,
                    "No machine found yet. Look again, or add one in the printer's settings.",
                );
            }
            list(ui, |ui| {
                for target in &targets {
                    let lit = bound.as_deref() == Some(target.key().as_str());
                    machine_row(ui, target, network.reach(target), lit);
                }
            });
            // TODO(step-8): add a machine by its address without leaving the plate.
            later(ui, |ui| secondary_button(ui, icon::ADD, "Add by address"));
        });
    if rescan {
        window.machine.network.scan();
    }
}

/// One machine: its name over where it is, and a dot for whether it answered.
fn machine_row(ui: &mut egui::Ui, target: &Target<'_>, reach: Reach, lit: bool) {
    let colors = theme::colors();
    let fill = if lit {
        colors.accent_wash
    } else {
        Color32::TRANSPARENT
    };
    egui::Frame::new()
        .fill(fill)
        .corner_radius(theme::R_CONTROL)
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(icon::PRINTER)
                        .font(theme::icon(16.0))
                        .color(colors.text_mid),
                );
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.label(
                        RichText::new(target.name())
                            .font(theme::label())
                            .color(colors.text_high),
                    );
                    ui.label(
                        RichText::new(target.detail())
                            .font(theme::code(10.5))
                            .color(colors.text_low),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    reach_dot(ui, reach);
                });
            });
        })
        .response
        .on_hover_text(reach.hint(&target.name()));
}
