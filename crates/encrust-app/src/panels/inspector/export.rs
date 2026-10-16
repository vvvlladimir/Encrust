use egui::{Color32, RichText};
use printer_profiles::Connection;

use crate::panels::Window;
use crate::panels::plate_summary::file_readings;
use crate::panels::printer_card::{self, Chosen};
use crate::panels::slice::{Run, Via, bound_machine, broken_reminder, start};
use crate::ui::{
    hint, icon, primary_button, readings, secondary_button, section, section_with_action, text_row,
    theme,
};
use crate::workspace::Tool;

/// From the plate to a file, and from the file to a printer, top to bottom. Slice in the
/// plate column does the same press; this is where its choices are read.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    file(ui, window);
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
        let what = "The file is on the machine. Nobody is standing at it yet.";
        if primary_button(ui, icon::PLAY, "Start the print", true)
            .on_hover_text(what)
            .clicked()
        {
            window
                .machine
                .network
                .start_print(&mut window.machine.status);
        }
        return;
    }

    let blocker = window.machine.slicing.blocker(&window.doc.scene);
    if blocker.is_none() {
        broken_reminder(ui, &window.doc.scene);
    }
    let busy = window.machine.slicing.job.is_some() || window.machine.network.job.is_some();
    let free = blocker.is_none() && !busy;
    let bound = bound_machine(window.machine, ui.input(|input| input.time));
    let plates = window.doc.scene.plates().len();
    let extension = window.machine.slicing.format.extension();
    let to_file = if every_plate(ui.ctx(), plates) {
        format!("Slice all {plates} plates to .{extension}")
    } else {
        format!("Slice to .{extension}")
    };

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
            if let Some(why) = &bound.blocker {
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
        let run = match via {
            Via::File if every_plate(ui.ctx(), plates) => Run::EveryPlate,
            Via::File | Via::Printer => Run::ThisPlate,
        };
        start(
            &window.doc.scene,
            &mut window.machine.slicing,
            &mut window.machine.status,
            run,
            via,
            window.machine.general.sliced_dir.as_deref(),
        );
    }
}

/// What the file comes to, its name, how much of the project it holds, and the way to
/// how it is cut. What it is written as belongs to the printer's settings.
fn file(ui: &mut egui::Ui, window: &mut Window) {
    let rows = file_readings(ui.ctx(), window);
    let plates = window.doc.scene.plates().len();
    let mut to_print_settings = false;
    section(ui, "The file", None, |ui| {
        readings(ui, &rows);
        text_row(ui, "File name", &mut window.machine.slicing.file_stem);
        scope_row(ui, plates);
        to_print_settings =
            secondary_button(ui, icon::PARAMETERS, "Layers, exposure and edges").clicked();
    });
    if to_print_settings {
        crate::shortcuts::pick(window, Tool::PrintSettings);
    }
}

/// Where the press at the foot reads whether it slices this plate or every one.
fn every_plate_id() -> egui::Id {
    egui::Id::new("export-every-plate")
}

/// Whether the press at the foot slices every plate; never while there is one.
fn every_plate(ctx: &egui::Context, plates: usize) -> bool {
    plates > 1 && ctx.data(|data| data.get_temp(every_plate_id()).unwrap_or(false))
}

/// This plate or every plate, offered once there is more than one.
fn scope_row(ui: &mut egui::Ui, plates: usize) {
    if plates < 2 {
        return;
    }
    let mut every = every_plate(ui.ctx(), plates);
    let all = format!("All {plates} plates");
    let shown = if every { all.as_str() } else { "This plate" };
    picker_row(ui, "export-scope", "Slice", shown, |ui| {
        ui.selectable_value(&mut every, false, "This plate");
        ui.selectable_value(&mut every, true, &all);
    });
    ui.data_mut(|data| data.insert_temp(every_plate_id(), every));
}

/// A drop-down that names its choice, under a label.
fn picker_row(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    chosen: &str,
    choices: impl FnOnce(&mut egui::Ui),
) {
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
                .show_ui(ui, choices);
        });
    });
}

/// Where the printer in hand sends its files: one row naming the listed printer and
/// what it is doing, opening the others the profile lists, and beside it that printer's
/// sheet. Which printers a profile lists is its own network settings (ADR 0265).
fn send(ui: &mut egui::Ui, window: &mut Window) {
    if cfg!(target_arch = "wasm32") {
        return;
    }
    let connection = window
        .machine
        .slicing
        .printer
        .as_ref()
        .map_or(Connection::None, |profile| profile.connection);
    window
        .machine
        .network
        .keep_fresh(ui.input(|input| input.time));
    let mut set_up = false;
    let (rescan, chosen) = section_with_action(
        ui,
        "Send to a printer",
        (icon::RESET, "Look again"),
        |ui| {
            let network = &window.machine.network;
            if connection == Connection::None {
                hint(
                    ui,
                    "This printer's profile takes files on a stick; choose its network in its settings to send from here.",
                );
                return None;
            }
            if network.listed().is_empty() {
                hint(ui, "No printer of this model is listed to send to yet.");
                set_up = secondary_button(ui, icon::NETWORK, "Choose them in its network settings")
                    .clicked();
                return None;
            }
            let warning = network
                .target()
                .filter(|target| !target.speaks(connection))
                .map(|target| {
                    format!(
                        "{} speaks {}, but this profile sends over {}: set its network in the printer's settings.",
                        network.label(&target),
                        target.protocol().label(),
                        connection.label()
                    )
                });
            printer_card::destination(ui, network, warning)
        },
    );
    if rescan {
        window.machine.network.scan();
    }
    if set_up {
        crate::panels::open_machines(window.machine);
        window.machine.settings.tab = crate::settings::Tab::Network;
    }
    match chosen.flatten() {
        Some(Chosen::Pick(address)) => window.machine.network.choose(address),
        Some(Chosen::Open(key)) => window.machine.network.open = Some(key),
        None => {}
    }
}
