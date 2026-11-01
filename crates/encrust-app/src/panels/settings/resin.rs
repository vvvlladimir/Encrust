//! The resin form: the exposure and motion of one resin, on one machine.

mod buffer;
mod motion;

use printer_profiles::{
    Buffer, Limit, MaterialProfile, PriceUnit, PrinterLimits, PrinterTuning, ResinDetails, WaitMode,
};

use crate::i18n::tr;
use crate::panels::Window;
use crate::panels::edge_fields;
use crate::panels::settings::{compensation, form_card, name_row, report, settled};
use crate::settings::{Node, ResinDraft, rename_resin};
use crate::slicing::{MAX_LAYER_HEIGHT_MM, MIN_LAYER_HEIGHT_MM};
use crate::state::Machine;
use crate::ui::{
    Carried, DropdownFace, DropdownRow, Held, Segment, Segmented, carried_row, describe, dropdown,
    dropdown_menu, dropdown_row, field_label, hint, limit_verdict, limited_row, number_field,
    number_row, text_row, theme,
};

const FINE_STEP: f64 = 0.01;
const COARSE_STEP: f64 = 0.5;

/// The currencies a price can be quoted in.
const CURRENCIES: [&str; 6] = ["€", "$", "£", "₽", "¥", "₴"];

/// A dropdown holding a currency or `kg`, points: room for the symbol and its caret.
const SMALL_DROPDOWN_W: f32 = 64.0;

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
    let hold = machine_hold(window, &resin.printer);

    let mut renamed = None;
    form_card(ui, &tr!("resin-resin"), |ui| {
        renamed = name_row(ui, "resin", &current);
        if let Some(resin) = window.machine.settings.resin.as_mut() {
            details(ui, &mut resin.draft.values);
        }
    });
    if let Some(resin) = window.machine.settings.resin.as_mut() {
        cards(ui, resin, &printer, &hold);
    }
    if let Some(resin) = window
        .machine
        .settings
        .resin
        .as_mut()
        .filter(|_| hold.locked.compensation.is_none())
    {
        let values = &mut resin.draft.values.compensation;
        let calculators = &mut window.machine.settings.calculators;
        form_card(ui, &tr!("resin-compensation"), |ui| {
            compensation::rows(ui, values, calculators)
        });
    }

    if let Some(name) = renamed {
        rename(window, &name);
    } else if settled(ui) {
        autosave(window.machine);
    }
}

/// What the printer a resin sits under holds it to.
#[derive(Default)]
struct MachineHold {
    locked: PrinterTuning,
    limits: PrinterLimits,
    buffer: Option<Buffer>,
    /// The machine's masks carry no grey, so there is no edge to smooth.
    binary_only: bool,
}

fn machine_hold(window: &Window, printer: &str) -> MachineHold {
    window
        .machine
        .slicing
        .catalogue
        .printer(printer)
        .map(|entry| MachineHold {
            locked: entry.profile.locked.clone(),
            limits: entry.profile.limits.clone(),
            buffer: entry.profile.buffer,
            binary_only: entry.profile.display.binary_only,
        })
        .unwrap_or_default()
}

/// The exposure, waits, motion and buffer cards, each where the printer leaves it open.
fn cards(ui: &mut egui::Ui, resin: &mut ResinDraft, printer: &str, hold: &MachineHold) {
    let MachineHold {
        locked,
        limits,
        buffer,
        binary_only,
    } = hold;
    form_card(ui, &tr!("resin-exposure-on", printer = printer), |ui| {
        describe(ui, &tr!("print-exposure-help"));
        light(ui, resin, locked, limits);
    });
    let values = &mut resin.draft.values;
    if !locked.locks_waiting() {
        form_card(ui, &tr!("resin-waits"), |ui| {
            describe(ui, &tr!("print-waiting-help"));
            waits(ui, values, locked, limits);
        });
    }
    if !locked.locks_motion() {
        form_card(ui, &tr!("print-motion"), |ui| {
            describe(ui, &tr!("print-motion-help"));
            motion::rows(ui, values, locked, limits);
        });
    }
    if let Some(machine) = *buffer {
        form_card(ui, &tr!("resin-buffer"), |ui| {
            describe(ui, &tr!("resin-buffer-help"));
            buffer::rows(ui, values, machine);
        });
    }
    if !binary_only {
        form_card(ui, &tr!("layers-edge-smoothing"), |ui| {
            describe(ui, &tr!("layers-edge-note"));
            edge_fields::rows(ui, &mut values.edges);
        });
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
    if report(&mut machine.status, &tr!("resin-cannot-save"), outcome).is_none() {
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
        &tr!("resin-cannot-rename"),
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
    text_row(ui, &tr!("resins-type"), &mut values.details.kind);
    ui.horizontal(|ui| {
        field_label(ui, &tr!("resin-colour"), theme::colors().text_mid, "");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.color_edit_button_srgb(&mut values.details.color);
        });
    });
    number_row(
        ui,
        &tr!("resin-density"),
        &mut values.density_g_cm3,
        &tr!("unit-g-ml"),
        FINE_STEP,
        0.1..=3.0,
        3,
    );
    price_row(ui, &mut values.details);
}

/// The price, the currency it is in and what it is a price of.
fn price_row(ui: &mut egui::Ui, details: &mut ResinDetails) {
    ui.horizontal(|ui| {
        field_label(ui, &tr!("summary-price"), theme::colors().text_mid, "");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let shown = unit_label(details.price_per);
            let face = DropdownFace {
                lead: None,
                label: &shown,
                edited: false,
                count: None,
            };
            let response = dropdown(ui, &face, SMALL_DROPDOWN_W);
            dropdown_menu(&response, |ui| {
                for unit in [PriceUnit::Kilogram, PriceUnit::Litre] {
                    let name = unit_label(unit);
                    let row = DropdownRow::named(&name, details.price_per == unit);
                    if dropdown_row(ui, &row).clicked() {
                        details.price_per = unit;
                        ui.close();
                    }
                }
            });
            ui.label(egui::RichText::new(tr!("resin-per")).color(theme::colors().text_low));
            let face = DropdownFace {
                lead: None,
                label: &details.currency,
                edited: false,
                count: None,
            };
            let response = dropdown(ui, &face, SMALL_DROPDOWN_W);
            dropdown_menu(&response, |ui| {
                for currency in CURRENCIES {
                    let row = DropdownRow::named(currency, details.currency == currency);
                    if dropdown_row(ui, &row).clicked() {
                        details.currency = currency.to_owned();
                        ui.close();
                    }
                }
            });
            number_field(
                ui,
                &tr!("summary-price"),
                &mut details.price,
                0.1,
                0.0..=10_000.0,
                Some(2),
                theme::FIELD_W,
            );
        });
    });
}

fn unit_label(unit: PriceUnit) -> String {
    match unit {
        PriceUnit::Kilogram => tr!("resin-per-kg"),
        PriceUnit::Litre => tr!("resin-per-litre"),
    }
}

/// How the printer waits around each layer: one light-off delay, or rests at the three
/// points of a peel. What the printer locks is not offered.
fn waits(
    ui: &mut egui::Ui,
    values: &mut MaterialProfile,
    locked: &PrinterTuning,
    limits: &PrinterLimits,
) {
    if locked.held_wait_mode().is_none() {
        let segments = [
            Segment::new(WaitMode::LightOff, tr!("resin-light-off-delay")),
            Segment::new(WaitMode::Rest, tr!("resin-rest-times")),
        ];
        let width = ui.available_width();
        Segmented::new(&segments)
            .width(width)
            .show(ui, &mut values.waits.mode);
        ui.add_space(4.0);
    }
    match locked.held_wait_mode().unwrap_or(values.waits.mode) {
        WaitMode::LightOff => {
            if locked.light_off_delay_s.is_none() {
                let held = Held::unexplained(&limits.light_off_delay_s);
                held_seconds_row(
                    ui,
                    &tr!("print-light-off"),
                    &mut values.light_off_delay_s,
                    held,
                );
            }
            if locked.bottom_light_off_delay_s.is_none() {
                let was = values
                    .bottom_light_off_delay_s
                    .unwrap_or(values.light_off_delay_s);
                let mut bottom = was;
                let held = Held::unexplained(&limits.bottom_light_off_delay_s);
                held_seconds_row(ui, &tr!("resin-bottom-light-off"), &mut bottom, held);
                if bottom != was {
                    values.bottom_light_off_delay_s = Some(bottom);
                }
            }
        }
        WaitMode::Rest => {
            if locked.waits.is_none() {
                let waits = &mut values.waits;
                seconds_row(ui, &tr!("print-before-lift"), &mut waits.before_lift_s);
                seconds_row(ui, &tr!("print-after-lift"), &mut waits.after_lift_s);
                seconds_row(ui, &tr!("print-after-retract"), &mut waits.after_retract_s);
            }
        }
    }
    hint(ui, &tr!("resin-waits-note"));
}

fn seconds_row(ui: &mut egui::Ui, label: &str, value: &mut f32) {
    number_row(ui, label, value, &tr!("unit-s"), FINE_STEP, 0.0..=60.0, 2);
}

fn held_seconds_row(ui: &mut egui::Ui, label: &str, value: &mut f32, held: Held<'_>) {
    let unit = tr!("unit-s");
    limited_row(
        ui,
        label,
        value,
        &unit,
        FINE_STEP,
        0.0..=60.0,
        Some(2),
        held,
    );
}

/// Exposure: how long each layer is lit, and how hard. The exposure is measured at the
/// layer height above it and follows it, saying so until it is touched.
fn light(
    ui: &mut egui::Ui,
    resin: &mut ResinDraft,
    locked: &PrinterTuning,
    limits: &PrinterLimits,
) {
    if locked.layer_height_mm.is_none() {
        layer_height_row(ui, resin, &limits.layer_height_mm);
    }
    let was = resin
        .carried()
        .map(|carried| (carried.exposure_s, carried.from_mm));
    let values = &mut resin.draft.values;
    if locked.exposure_s.is_some() {
        bottom_and_power(ui, values, locked, limits);
        return;
    }
    let carried = exposure_row(ui, values, &limits.exposure_s, was);
    bottom_and_power(ui, values, locked, limits);
    match carried {
        Carried::Edited => resin.exposure_edited(),
        Carried::Reverted => resin.revert_exposure(),
        Carried::Untouched => {}
    }
}

fn layer_height_row(ui: &mut egui::Ui, resin: &mut ResinDraft, limit: &Limit) {
    let mut layer_height_mm = resin.draft.values.layer_height_mm;
    if limited_row(
        ui,
        &tr!("resin-layer-height"),
        &mut layer_height_mm,
        &tr!("unit-mm"),
        FINE_STEP,
        MIN_LAYER_HEIGHT_MM..=MAX_LAYER_HEIGHT_MM,
        Some(3),
        Held::layer_height(limit),
    ) {
        resin.set_layer_height(layer_height_mm);
    }
}

/// The normal exposure, its verdict, and where it was carried from, `was` holding the
/// exposure and the height it was measured at.
fn exposure_row(
    ui: &mut egui::Ui,
    values: &mut MaterialProfile,
    limit: &Limit,
    was: Option<(f32, f32)>,
) -> Carried {
    let unit = tr!("unit-s");
    let carried = carried_row(
        ui,
        &tr!("print-exposure"),
        &mut values.exposure_s,
        &unit,
        FINE_STEP,
        limit.range(0.05..=120.0),
        2,
        was.map(|(exposure_s, _)| exposure_s),
    );
    limit_verdict(
        ui,
        Held::unexplained(limit),
        values.exposure_s,
        &unit,
        Some(2),
    );
    if let Some((exposure_s, from_mm)) = was {
        hint(
            ui,
            &tr!(
                "resin-carried",
                exposure = format!("{exposure_s:.2}"),
                from = format!("{from_mm:.3}"),
                to = format!("{:.3}", values.layer_height_mm)
            ),
        );
    }
    carried
}

/// The bottom block, the ramp out of it and the light's power, where the printer leaves
/// them to the resin.
fn bottom_and_power(
    ui: &mut egui::Ui,
    values: &mut MaterialProfile,
    locked: &PrinterTuning,
    limits: &PrinterLimits,
) {
    if locked.bottom_exposure_s.is_none() {
        limited_row(
            ui,
            &tr!("print-bottom-exposure"),
            &mut values.bottom_exposure_s,
            &tr!("unit-s"),
            COARSE_STEP,
            0.05..=600.0,
            Some(2),
            Held::unexplained(&limits.bottom_exposure_s),
        );
    }
    if locked.bottom_layers.is_none() {
        let held = Held::bottom_layers(&limits.bottom_layers);
        let label = tr!("print-bottom-layers");
        limited_row(
            ui,
            &label,
            &mut values.bottom_layers,
            "",
            1.0,
            1..=200,
            None,
            held,
        );
    }
    if locked.transition_layers.is_none() {
        transition_row(ui, values, Held::unexplained(&limits.transition_layers));
    }
    if locked.light_pwm.is_none() {
        pwm_row(ui, &tr!("resin-light-power"), &mut values.light_pwm);
    }
    if locked.bottom_light_pwm.is_none() {
        pwm_row(ui, &tr!("resin-bottom-power"), &mut values.bottom_light_pwm);
    }
}

/// The transition block is a `u16` in the profile, and a count on screen like any other.
fn transition_row(ui: &mut egui::Ui, values: &mut MaterialProfile, held: Held<'_>) {
    let mut layers = u32::from(values.transition_layers);
    let label = tr!("print-transition-layers");
    if limited_row(ui, &label, &mut layers, "", 1.0, 0..=200, None, held) {
        values.transition_layers = layers as u16;
    }
    if values.transition_layers > 0 {
        let step = (values.bottom_exposure_s - values.exposure_s)
            / (f32::from(values.transition_layers) + 1.0);
        hint(
            ui,
            &tr!("resin-transition-note", step = format!("{step:.2}")),
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
