//! The printer form: the panel, the build volume, the file format a machine reads and the
//! machine on the network it is sent to.

mod connection;

use core_pipeline::SlicedFormat;
use printer_profiles::{Display, Mirror, PlateInset, PrinterProfile};

use crate::i18n::tr;
use crate::job::label_of;
use crate::panels::Window;
use crate::panels::settings::{form_card, report, settled};
use crate::ui::{
    count_row, describe, hint, icon, number_row, picker, picker_row, secondary_button, switch,
    text_row,
};

/// Millimetres per point of drag. A panel is measured in tenths, a build volume in whole
/// millimetres.
const FINE_STEP: f64 = 0.01;
const COARSE_STEP: f64 = 0.1;

/// The machine open in the list, written back as it is edited.
pub fn form(ui: &mut egui::Ui, window: &mut Window) {
    let Some(draft) = window.machine.settings.printer.as_mut() else {
        return;
    };
    let shipped = draft.shipped();
    let (id, profile) = (draft.id.clone(), &mut draft.values);
    let mut restore = false;
    form_card(ui, &tr!("settings-tab-machine"), |ui| {
        text_row(ui, &tr!("settings-name"), &mut profile.name);
        text_row(ui, &tr!("printer-manufacturer"), &mut profile.manufacturer);
        if shipped.is_some() {
            restore = own_copy_note(ui);
        }
    });
    form_card(ui, &tr!("printer-display"), |ui| display(ui, profile));
    form_card(ui, &tr!("printer-build-volume"), |ui| volume(ui, profile));
    form_card(ui, &tr!("printer-output"), |ui| output(ui, profile));
    form_card(ui, &tr!("printer-firmware"), |ui| firmware(ui, profile));

    if restore {
        if let Some(shipped) = shipped {
            put_shipped_back(window, &id, shipped);
        }
        return;
    }
    if settled(ui) {
        autosave(window);
    }
}

/// What the machine open in the list takes a file over, written back as it is edited.
pub fn network(ui: &mut egui::Ui, window: &mut Window) {
    let machine = &mut *window.machine;
    let Some(draft) = machine.settings.printer.as_mut() else {
        return;
    };
    let profile = &mut draft.values;
    // A browser reaches no printer, so a file is downloaded instead (ADR 0182).
    form_card(ui, &tr!("settings-tab-network"), |ui| {
        match cfg!(target_arch = "wasm32") {
            true => hint(ui, &tr!("printer-browser-sends-nothing")),
            false => connection::card(ui, profile, &mut machine.network),
        }
    });
    if settled(ui) {
        autosave(window);
    }
}

/// Says this machine is the user's copy of a shipped one and no longer matches it (P-11).
/// Answers whether the shipped profile was asked for back.
fn own_copy_note(ui: &mut egui::Ui) -> bool {
    ui.add_space(4.0);
    hint(ui, &tr!("printer-own-copy"));
    ui.add_space(4.0);
    secondary_button(ui, icon::UPDATE, &tr!("printer-use-shipped")).clicked()
}

/// Writes the shipped profile back over the user's copy, and stands the plate under it
/// when it is the machine in hand.
fn put_shipped_back(window: &mut Window, id: &str, shipped: PrinterProfile) {
    let outcome = window
        .machine
        .slicing
        .catalogue
        .save_printer(id, &shipped)
        .map(drop);
    if report(
        &mut window.machine.status,
        &tr!("printer-cannot-put-back"),
        outcome,
    )
    .is_none()
    {
        return;
    }
    let node = crate::settings::Node::Printer(id.to_owned());
    window
        .machine
        .settings
        .pick(&window.machine.slicing.catalogue, node);
    if window.machine.slicing.printer_id.as_deref() == Some(id) {
        crate::profiles::apply_printer(window, shipped, Some(id.to_owned()));
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
        &tr!("printer-cannot-save"),
        outcome,
    )
    .is_none()
    {
        return;
    }
    draft.mark_saved();
    let (id, profile) = (draft.id.clone(), draft.values.clone());
    crate::profiles::learn_printer_names(window.machine);
    if window.machine.slicing.printer_id.as_deref() == Some(id.as_str()) {
        crate::profiles::printer_edited(window, profile, id);
    }
}

/// The greys an edge is spread over on this machine; see `docs/decisions/0378`.
fn grey_range_rows(ui: &mut egui::Ui, display: &mut Display) {
    let mut min = u32::from(display.grey_min);
    let mut max = u32::from(display.grey_max);
    if count_row(ui, &tr!("printer-grey-min"), &mut min, "", 1.0, 0..=max) {
        display.grey_min = min as u8;
    }
    if count_row(ui, &tr!("printer-grey-max"), &mut max, "", 1.0, min..=255) {
        display.grey_max = max as u8;
    }
    hint(
        ui,
        &match (display.grey_min, display.grey_max) {
            (0, 255) => tr!("printer-grey-range-full"),
            _ => tr!("printer-grey-range-spread"),
        },
    );
}

/// The panel, the pitch it works out to, and the greys an edge is spread over.
fn display(ui: &mut egui::Ui, profile: &mut PrinterProfile) {
    count_row(
        ui,
        &tr!("printer-panel-width"),
        &mut profile.display.width_px,
        &tr!("unit-px"),
        1.0,
        1..=u32::MAX,
    );
    count_row(
        ui,
        &tr!("printer-panel-height"),
        &mut profile.display.height_px,
        &tr!("unit-px"),
        1.0,
        1..=u32::MAX,
    );
    number_row(
        ui,
        &tr!("printer-lit-width"),
        &mut profile.display.width_mm,
        &tr!("unit-mm"),
        FINE_STEP,
        0.01..=1000.0,
        2,
    );
    number_row(
        ui,
        &tr!("printer-lit-height"),
        &mut profile.display.height_mm,
        &tr!("unit-mm"),
        FINE_STEP,
        0.01..=1000.0,
        2,
    );

    let (pitch_x, pitch_y) = profile.display.pixel_pitch_mm();
    hint(
        ui,
        &tr!(
            "printer-pitch",
            x = format!("{:.1}", pitch_x * 1000.0),
            y = format!("{:.1}", pitch_y * 1000.0)
        ),
    );

    grey_range_rows(ui, &mut profile.display);
}

/// The envelope the plate can print in, and which way round the panel is mounted.
fn volume(ui: &mut egui::Ui, profile: &mut PrinterProfile) {
    number_row(
        ui,
        &tr!("printer-volume-x"),
        &mut profile.build_volume.x,
        &tr!("unit-mm"),
        COARSE_STEP,
        1.0..=1000.0,
        2,
    );
    number_row(
        ui,
        &tr!("printer-volume-y"),
        &mut profile.build_volume.y,
        &tr!("unit-mm"),
        COARSE_STEP,
        1.0..=1000.0,
        2,
    );
    number_row(
        ui,
        &tr!("printer-volume-z"),
        &mut profile.build_volume.z,
        &tr!("unit-mm"),
        COARSE_STEP,
        1.0..=1000.0,
        2,
    );

    ui.add_space(6.0);
    mirror(ui, &mut profile.mirror);

    ui.add_space(6.0);
    let plate_mm = (profile.build_volume.x, profile.build_volume.y);
    inset(ui, &mut profile.plate_inset, plate_mm);
}

/// The band along the plate's edges a model is marked in, each edge on its own; off is none.
/// A band takes at most half the plate across it, so two never meet past the middle.
fn inset(ui: &mut egui::Ui, inset: &mut Option<PlateInset>, (width_mm, depth_mm): (f32, f32)) {
    let mut on = inset.is_some();
    if switch(ui, &mut on, &tr!("printer-inset")).changed() {
        *inset = on.then(PlateInset::default);
    }
    if let Some(inset) = inset {
        let (across_x, across_y) = (width_mm / 2.0, depth_mm / 2.0);
        for (label, edge, most_mm) in [
            (tr!("printer-inset-min-x"), &mut inset.min_x_mm, across_x),
            (tr!("printer-inset-max-x"), &mut inset.max_x_mm, across_x),
            (tr!("printer-inset-min-y"), &mut inset.min_y_mm, across_y),
            (tr!("printer-inset-max-y"), &mut inset.max_y_mm, across_y),
        ] {
            let most_mm = most_mm.clamp(0.0, 50.0);
            number_row(
                ui,
                &label,
                edge,
                &tr!("unit-mm"),
                FINE_STEP,
                0.0..=most_mm,
                2,
            );
        }
    }
    describe(ui, &tr!("printer-inset-note"));
}

/// What this machine obeys beyond the header, which no file format states.
fn firmware(ui: &mut egui::Ui, profile: &mut PrinterProfile) {
    switch(
        ui,
        &mut profile.firmware.per_layer_settings,
        &tr!("printer-per-layer"),
    );
    hint(
        ui,
        &match profile.firmware.per_layer_settings {
            true => tr!("printer-per-layer-on"),
            false => tr!("printer-per-layer-off"),
        },
    );

    ui.add_space(6.0);
    switch(
        ui,
        &mut profile.firmware.variable_layer_height,
        &tr!("printer-variable-height"),
    );
    hint(ui, &tr!("printer-variable-height-note"));
}

/// How the panel takes the plate, as the maker's settings state it.
fn mirror(ui: &mut egui::Ui, mirror: &mut Mirror) {
    let response = picker_row(
        ui,
        &tr!("printer-mirror"),
        icon::FLIP,
        &mirror_label(*mirror),
    );
    egui::Popup::menu(&response).show(|ui| {
        for choice in Mirror::ALL {
            if ui
                .selectable_label(choice == *mirror, mirror_label(choice))
                .clicked()
            {
                *mirror = choice;
                ui.close();
            }
        }
    });
    hint(ui, &tr!("printer-mirror-note"));
}

fn mirror_label(mirror: Mirror) -> String {
    match mirror {
        Mirror::Normal => tr!("printer-mirror-normal"),
        Mirror::LcdMirror => tr!("printer-mirror-lcd"),
        Mirror::DlpNormal => tr!("printer-mirror-dlp"),
    }
}

/// The container this machine's firmware reads. Only containers the window can write for
/// this machine are listed: a `.cxdlp` needs a CL or CT code in the name.
fn output(ui: &mut egui::Ui, profile: &mut PrinterProfile) {
    let chosen = SlicedFormat::from(profile.output);
    let offered: Vec<SlicedFormat> = SlicedFormat::choices_for(profile).collect();
    let response = picker(ui, icon::SLICE, label_of(chosen));
    egui::Popup::menu(&response).show(|ui| {
        for choice in offered {
            if ui
                .selectable_label(choice == chosen, label_of(choice))
                .clicked()
            {
                profile.output = choice.into();
                ui.close();
            }
        }
    });
    hint(ui, &tr!("printer-output-note"));
}
