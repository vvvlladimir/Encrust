//! The resin form: the exposure and motion of one resin, on one machine.

use printer_profiles::{MaterialProfile, PriceUnit, ResinDetails, WaitMode};

use crate::panels::Window;
use crate::panels::settings::{compensation, form_card, name_row, report, settled};
use crate::settings::{Node, ResinDraft, rename_resin};
use crate::slicing::{MAX_LAYER_HEIGHT_MM, MIN_LAYER_HEIGHT_MM};
use crate::state::Machine;
use crate::ui::{
    Carried, Segment, Segmented, carried_row, field_label, hint, number_field, number_row,
    text_row, theme,
};

const FINE_STEP: f64 = 0.01;
const COARSE_STEP: f64 = 0.5;

/// The currencies a price can be quoted in.
const CURRENCIES: [&str; 6] = ["€", "$", "£", "₽", "¥", "₴"];

/// The resin open in the list, as the printer it sits under has it, written back as it
/// is edited.
pub fn form(ui: &mut egui::Ui, window: &mut Window) {
    let Some(resin) = window.machine.settings.resin.as_ref() else {
        return;
    };
    let printer = window
        .machine
        .settings
        .printer
        .as_ref()
        .map_or_else(String::new, |printer| printer.values.name.clone());
    let current = resin.draft.values.name.clone();

    let mut renamed = None;
    form_card(ui, "Resin", |ui| {
        renamed = name_row(ui, "resin", &current);
        if let Some(resin) = window.machine.settings.resin.as_mut() {
            details(ui, &mut resin.draft.values);
        }
    });
    if let Some(resin) = window.machine.settings.resin.as_mut() {
        form_card(ui, &format!("Exposure on the {printer}"), |ui| {
            light(ui, resin);
        });
        let values = &mut resin.draft.values;
        form_card(ui, "Waits", |ui| waits(ui, values));
        form_card(ui, "Motion", |ui| motion(ui, values));
    }
    let pixel_mm = window
        .machine
        .settings
        .printer
        .as_ref()
        .map_or(0.0, |printer| {
            printer.values.display.width_mm / printer.values.display.width_px as f32
        });
    if let Some(resin) = window.machine.settings.resin.as_mut() {
        let values = &mut resin.draft.values.compensation;
        let calculators = &mut window.machine.settings.calculators;
        form_card(ui, "Compensation", |ui| {
            compensation::rows(ui, values, calculators, pixel_mm)
        });
    }

    if let Some(name) = renamed {
        rename(window, &name);
    } else if settled(ui) {
        autosave(window.machine);
    }
}

/// Writes this printer's numbers when they changed, and hands them to the plate when it
/// prints with this resin.
fn autosave(machine: &mut Machine) {
    let Some(resin) = machine.settings.resin.as_mut() else {
        return;
    };
    if !resin.draft.is_dirty() {
        return;
    }
    let saved = resin.to_saved();
    let outcome = machine
        .slicing
        .catalogue
        .save_resin(&resin.draft.id, &saved);
    if report(&mut machine.status, "cannot save the resin", outcome).is_none() {
        return;
    }
    resin.base = saved;
    resin.draft.mark_saved();
    machine.slicing.reload_resin();
}

/// Gives the resin a new name on this printer, which may split it off the others.
fn rename(window: &mut Window, name: &str) {
    autosave(window.machine);
    let Some(resin) = window.machine.settings.resin.as_ref() else {
        return;
    };
    let (printer, old) = (resin.printer.clone(), resin.draft.id.clone());
    let outcome = rename_resin(&mut window.machine.slicing.catalogue, &printer, &old, name);
    let Some(id) = report(
        &mut window.machine.status,
        "cannot rename the resin",
        outcome,
    ) else {
        return;
    };
    let in_use = window.machine.slicing.resin_id.as_deref() == Some(old.as_str())
        && window.machine.slicing.printer_id.as_deref() == Some(printer.as_str());
    if in_use {
        window.machine.slicing.resin_id = Some(id.clone());
    }
    window.machine.slicing.reload_resin();
    let node = Node::Resin { printer, resin: id };
    window
        .machine
        .settings
        .pick(&window.machine.slicing.catalogue, node);
}

/// What the resin is, what it looks like and what it costs.
fn details(ui: &mut egui::Ui, values: &mut MaterialProfile) {
    text_row(ui, "Type", &mut values.details.kind);
    ui.horizontal(|ui| {
        field_label(ui, "Colour", theme::colors().text_mid, "");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.color_edit_button_srgb(&mut values.details.color);
        });
    });
    number_row(
        ui,
        "Density",
        &mut values.density_g_cm3,
        "g/ml",
        FINE_STEP,
        0.1..=3.0,
        3,
    );
    price_row(ui, &mut values.details);
}

/// The price, the currency it is in and what it is a price of.
fn price_row(ui: &mut egui::Ui, details: &mut ResinDetails) {
    ui.horizontal(|ui| {
        field_label(ui, "Price", theme::colors().text_mid, "");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            egui::ComboBox::from_id_salt("resin-price-per")
                .width(56.0)
                .selected_text(unit_label(details.price_per))
                .show_ui(ui, |ui| {
                    for unit in [PriceUnit::Kilogram, PriceUnit::Litre] {
                        ui.selectable_value(&mut details.price_per, unit, unit_label(unit));
                    }
                });
            ui.label(egui::RichText::new("per").color(theme::colors().text_low));
            egui::ComboBox::from_id_salt("resin-currency")
                .width(44.0)
                .selected_text(details.currency.clone())
                .show_ui(ui, |ui| {
                    for currency in CURRENCIES {
                        ui.selectable_value(&mut details.currency, currency.to_owned(), currency);
                    }
                });
            number_field(
                ui,
                &mut details.price,
                0.1,
                0.0..=10_000.0,
                Some(2),
                theme::FIELD_W,
            );
        });
    });
}

fn unit_label(unit: PriceUnit) -> &'static str {
    match unit {
        PriceUnit::Kilogram => "kg",
        PriceUnit::Litre => "l",
    }
}

/// How the printer waits around each layer: one light-off delay, or rests at the three
/// points of a peel.
fn waits(ui: &mut egui::Ui, values: &mut MaterialProfile) {
    let segments = [
        Segment::new(WaitMode::LightOff, "Light-off delay"),
        Segment::new(WaitMode::Rest, "Rest times"),
    ];
    let width = ui.available_width();
    Segmented::new(&segments)
        .width(width)
        .show(ui, &mut values.waits.mode);
    ui.add_space(4.0);
    match values.waits.mode {
        WaitMode::LightOff => {
            number_row(
                ui,
                "Light off",
                &mut values.light_off_delay_s,
                "s",
                FINE_STEP,
                0.0..=60.0,
                2,
            );
        }
        WaitMode::Rest => {
            let waits = &mut values.waits;
            number_row(
                ui,
                "Before lift",
                &mut waits.before_lift_s,
                "s",
                FINE_STEP,
                0.0..=60.0,
                2,
            );
            number_row(
                ui,
                "After lift",
                &mut waits.after_lift_s,
                "s",
                FINE_STEP,
                0.0..=60.0,
                2,
            );
            number_row(
                ui,
                "After retract",
                &mut waits.after_retract_s,
                "s",
                FINE_STEP,
                0.0..=60.0,
                2,
            );
        }
    }
    hint(
        ui,
        "Counted into the print time, and written into the file.",
    );
}

/// Exposure: how long each layer is lit, and how hard. The exposure is measured at the
/// layer height above it and follows it, saying so until it is touched.
fn light(ui: &mut egui::Ui, resin: &mut ResinDraft) {
    let mut layer_height_mm = resin.draft.values.layer_height_mm;
    if number_row(
        ui,
        "Layer height",
        &mut layer_height_mm,
        "mm",
        FINE_STEP,
        MIN_LAYER_HEIGHT_MM..=MAX_LAYER_HEIGHT_MM,
        3,
    ) {
        resin.set_layer_height(layer_height_mm);
    }
    let was = resin
        .carried()
        .map(|carried| (carried.exposure_s, carried.from_mm));
    let values = &mut resin.draft.values;
    let carried = carried_row(
        ui,
        "Exposure",
        &mut values.exposure_s,
        "s",
        FINE_STEP,
        0.05..=120.0,
        2,
        was.map(|(exposure_s, _)| exposure_s),
    );
    if let Some((exposure_s, from_mm)) = was {
        hint(
            ui,
            &format!(
                "Carried from {exposure_s:.2} s at {from_mm:.3} mm to {:.3} mm.",
                values.layer_height_mm
            ),
        );
    }
    number_row(
        ui,
        "Bottom exposure",
        &mut values.bottom_exposure_s,
        "s",
        COARSE_STEP,
        0.05..=600.0,
        2,
    );
    whole_row(ui, "Bottom layers", &mut values.bottom_layers, 1.0, 1..=200);
    transition_row(ui, values);
    pwm_row(ui, "Light power", &mut values.light_pwm);
    pwm_row(ui, "Bottom power", &mut values.bottom_light_pwm);
    match carried {
        Carried::Edited => resin.exposure_edited(),
        Carried::Reverted => resin.revert_exposure(),
        Carried::Untouched => {}
    }
}

/// Motion: how the plate peels the layer off the film and comes back.
fn motion(ui: &mut egui::Ui, values: &mut MaterialProfile) {
    number_row(
        ui,
        "Lift",
        &mut values.lift_distance_mm,
        "mm",
        COARSE_STEP,
        0.1..=100.0,
        2,
    );
    number_row(
        ui,
        "Lift speed",
        &mut values.lift_speed_mm_min,
        "mm/min",
        1.0,
        1.0..=1000.0,
        1,
    );
    number_row(
        ui,
        "Retract",
        &mut values.retract_distance_mm,
        "mm",
        COARSE_STEP,
        0.1..=100.0,
        2,
    );
    number_row(
        ui,
        "Retract speed",
        &mut values.retract_speed_mm_min,
        "mm/min",
        1.0,
        1.0..=1000.0,
        1,
    );
    ui.add_space(8.0);
    number_row(
        ui,
        "Bottom lift",
        &mut values.bottom_lift_distance_mm,
        "mm",
        COARSE_STEP,
        0.1..=100.0,
        2,
    );
    number_row(
        ui,
        "Bottom lift speed",
        &mut values.bottom_lift_speed_mm_min,
        "mm/min",
        1.0,
        1.0..=1000.0,
        1,
    );
    number_row(
        ui,
        "Bottom retract speed",
        &mut values.bottom_retract_speed_mm_min,
        "mm/min",
        1.0,
        1.0..=1000.0,
        1,
    );
}

fn whole_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut u32,
    speed: f64,
    range: std::ops::RangeInclusive<u32>,
) {
    crate::ui::count_row(ui, label, value, "", speed, range);
}

/// The transition block is a `u16` in the profile, and a count on screen like any other.
fn transition_row(ui: &mut egui::Ui, values: &mut MaterialProfile) {
    let mut layers = u32::from(values.transition_layers);
    if crate::ui::count_row(ui, "Transition layers", &mut layers, "", 1.0, 0..=200) {
        values.transition_layers = layers as u16;
    }
    if values.transition_layers > 0 {
        let step = (values.bottom_exposure_s - values.exposure_s)
            / (f32::from(values.transition_layers) + 1.0);
        hint(
            ui,
            &format!("Linear: each layer {step:.2} s shorter than the one under it."),
        );
    }
}

/// UV power runs 0 to 255, which is a byte in the profile and a count on screen.
fn pwm_row(ui: &mut egui::Ui, label: &str, value: &mut u8) {
    let mut power = u32::from(*value);
    if crate::ui::count_row(ui, label, &mut power, "", 1.0, 0..=255) {
        *value = power as u8;
    }
}
