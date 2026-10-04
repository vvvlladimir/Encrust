//! The printer form: the panel, the build volume, the file format a machine reads and the
//! machine on the network it is sent to.

mod connection;

use core_pipeline::SlicedFormat;
use printer_profiles::PrinterProfile;

use crate::job::label_of;
use crate::panels::Window;
use crate::panels::settings::{form_card, report, settled};
use crate::ui::{count_row, hint, icon, number_row, picker, switch, text_row};

/// Millimetres per point of drag. A panel is measured in tenths, a build volume in whole
/// millimetres.
const FINE_STEP: f64 = 0.01;
const COARSE_STEP: f64 = 0.1;

/// The machine open in the list, written back as it is edited.
pub fn form(ui: &mut egui::Ui, window: &mut Window) {
    let machine = &mut *window.machine;
    let Some(draft) = machine.settings.printer.as_mut() else {
        return;
    };
    let (id, profile) = (draft.id.clone(), &mut draft.values);
    form_card(ui, "Machine", |ui| {
        text_row(ui, "Name", &mut profile.name);
        text_row(ui, "Manufacturer", &mut profile.manufacturer);
    });
    form_card(ui, "Display", |ui| display(ui, profile));
    form_card(ui, "Build volume", |ui| volume(ui, profile));
    form_card(ui, "Output", |ui| output(ui, profile));
    // TODO(step-A6): a browser reaches no printer yet, so it has nothing to set up here.
    if cfg!(not(target_arch = "wasm32")) {
        form_card(ui, "Network", |ui| {
            connection::card(ui, &id, &mut profile.connection, &mut machine.network);
        });
    }
    form_card(ui, "Firmware", |ui| firmware(ui, profile));

    if settled(ui) {
        autosave(window);
    }
}

/// Writes the printer when it changed, and stands the plate back under it when it is the
/// printer in use and the change moves the plate.
fn autosave(window: &mut Window) {
    let Some(draft) = window.machine.settings.printer.as_mut() else {
        return;
    };
    if !draft.is_dirty() {
        return;
    }
    let outcome = window
        .machine
        .slicing
        .catalogue
        .save_printer(&draft.id, &draft.values);
    if report(
        &mut window.machine.status,
        "cannot save the printer",
        outcome,
    )
    .is_none()
    {
        return;
    }
    draft.mark_saved();
    let (id, profile) = (draft.id.clone(), draft.values.clone());
    if window.machine.slicing.printer_id.as_deref() != Some(id.as_str()) {
        return;
    }
    let moves_the_plate = window.machine.slicing.printer.as_ref().is_none_or(|old| {
        old.build_volume != profile.build_volume
            || old.display != profile.display
            || old.output != profile.output
    });
    if moves_the_plate {
        crate::profiles::apply_printer(window, profile, Some(id));
    } else {
        window.machine.slicing.printer = Some(profile);
    }
}

/// The dimmest grey this panel cures. Below it a mask pixel is written black instead of
/// being left as resin that never sets; see `docs/decisions/0113`.
fn grey_floor_row(ui: &mut egui::Ui, floor: &mut u8) {
    let mut value = u32::from(*floor);
    if count_row(ui, "Grey floor", &mut value, "", 1.0, 0..=254) {
        *floor = value as u8;
    }
    hint(
        ui,
        match floor {
            0 => "every grey is written; a pixel too dim to cure leaves liquid resin",
            _ => "a dimmer pixel is written black, so an edge loses a fraction of a pixel",
        },
    );
}

/// The panel, the pitch it works out to, and the dimmest grey it cures.
fn display(ui: &mut egui::Ui, profile: &mut PrinterProfile) {
    count_row(
        ui,
        "Panel width",
        &mut profile.display.width_px,
        "px",
        1.0,
        1..=u32::MAX,
    );
    count_row(
        ui,
        "Panel height",
        &mut profile.display.height_px,
        "px",
        1.0,
        1..=u32::MAX,
    );
    number_row(
        ui,
        "Lit width",
        &mut profile.display.width_mm,
        "mm",
        FINE_STEP,
        0.01..=1000.0,
        2,
    );
    number_row(
        ui,
        "Lit height",
        &mut profile.display.height_mm,
        "mm",
        FINE_STEP,
        0.01..=1000.0,
        2,
    );

    let (pitch_x, pitch_y) = profile.display.pixel_pitch_mm();
    hint(
        ui,
        &format!(
            "{:.1} x {:.1} um a pixel",
            pitch_x * 1000.0,
            pitch_y * 1000.0
        ),
    );

    grey_floor_row(ui, &mut profile.display.grey_floor);
}

/// The envelope the plate can print in, and which way round the panel is mounted.
fn volume(ui: &mut egui::Ui, profile: &mut PrinterProfile) {
    number_row(
        ui,
        "Volume X",
        &mut profile.build_volume.x,
        "mm",
        COARSE_STEP,
        1.0..=1000.0,
        2,
    );
    number_row(
        ui,
        "Volume Y",
        &mut profile.build_volume.y,
        "mm",
        COARSE_STEP,
        1.0..=1000.0,
        2,
    );
    number_row(
        ui,
        "Volume Z",
        &mut profile.build_volume.z,
        "mm",
        COARSE_STEP,
        1.0..=1000.0,
        2,
    );

    ui.add_space(6.0);
    switch(ui, &mut profile.mirror_x, "Mirror across X");
    switch(ui, &mut profile.mirror_y, "Mirror across Y");
    hint(ui, "A mask printed the wrong way round is a mirrored part.");
}

/// What this machine obeys beyond the header, which no file format states.
fn firmware(ui: &mut egui::Ui, profile: &mut PrinterProfile) {
    switch(
        ui,
        &mut profile.firmware.per_layer_settings,
        "Per-layer settings",
    );
    hint(
        ui,
        match profile.firmware.per_layer_settings {
            true => "exposure and motion are read from each layer; .goo calls this advance mode",
            false => {
                "every layer prints at the header's numbers, and the machine ramps the \
                      bottom block itself"
            }
        },
    );

    ui.add_space(6.0);
    switch(
        ui,
        &mut profile.firmware.variable_layer_height,
        "Variable layer height",
    );
    hint(
        ui,
        "Off means the plate steps by the header's height, so an adaptive stack is refused.",
    );
}

/// The container this machine's firmware reads. Every one the window writes is on the
/// list: a machine reads one of them and the eight do not fit across a card.
fn output(ui: &mut egui::Ui, profile: &mut PrinterProfile) {
    let chosen = SlicedFormat::from(profile.output);
    let response = picker(ui, icon::SLICE, label_of(chosen));
    egui::Popup::menu(&response).show(|ui| {
        for choice in SlicedFormat::CHOICES {
            if ui
                .selectable_label(choice == chosen, label_of(choice))
                .clicked()
            {
                profile.output = choice.into();
                ui.close();
            }
        }
    });
    hint(
        ui,
        "What this machine is sliced into, dialog and printer alike.",
    );
}
