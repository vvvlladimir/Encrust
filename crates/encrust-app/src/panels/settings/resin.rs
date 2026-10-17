//! The resin form: the exposure and motion of one resin, on one machine.

use printer_profiles::{MaterialProfile, PriceUnit, PrinterTuning, ResinDetails, WaitMode};

use crate::i18n::tr;
use crate::panels::Window;
use crate::panels::settings::{compensation, form_card, name_row, report, settled};
use crate::settings::{Node, ResinDraft, rename_resin};
use crate::slicing::{MAX_LAYER_HEIGHT_MM, MIN_LAYER_HEIGHT_MM};
use crate::state::Machine;
use crate::ui::{
    Carried, Segment, Segmented, carried_row, describe, field_label, hint, number_field,
    number_row, text_row, theme,
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
    let locked = window
        .machine
        .slicing
        .catalogue
        .printer(&resin.printer)
        .map(|entry| entry.profile.locked.clone())
        .unwrap_or_default();

    let mut renamed = None;
    form_card(ui, &tr!("resin-resin"), |ui| {
        renamed = name_row(ui, "resin", &current);
        if let Some(resin) = window.machine.settings.resin.as_mut() {
            details(ui, &mut resin.draft.values);
        }
    });
    if let Some(resin) = window.machine.settings.resin.as_mut() {
        form_card(
            ui,
            &tr!("resin-exposure-on", printer = printer.clone()),
            |ui| {
                describe(ui, &tr!("print-exposure-help"));
                light(ui, resin, &locked);
            },
        );
        let values = &mut resin.draft.values;
        if shows_waits(&locked) {
            form_card(ui, &tr!("resin-waits"), |ui| {
                describe(ui, &tr!("print-waiting-help"));
                waits(ui, values, &locked);
            });
        }
        if !locks_all_motion(&locked) {
            form_card(ui, &tr!("print-motion"), |ui| {
                describe(ui, &tr!("print-motion-help"));
                motion(ui, values, &locked);
            });
        }
    }
    if let Some(resin) = window
        .machine
        .settings
        .resin
        .as_mut()
        .filter(|_| locked.compensation.is_none())
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
            egui::ComboBox::from_id_salt("resin-price-per")
                .width(56.0)
                .selected_text(unit_label(details.price_per))
                .show_ui(ui, |ui| {
                    for unit in [PriceUnit::Kilogram, PriceUnit::Litre] {
                        ui.selectable_value(&mut details.price_per, unit, unit_label(unit));
                    }
                });
            ui.label(egui::RichText::new(tr!("resin-per")).color(theme::colors().text_low));
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
fn waits(ui: &mut egui::Ui, values: &mut MaterialProfile, locked: &PrinterTuning) {
    if locked_mode(locked).is_none() {
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
    match locked_mode(locked).unwrap_or(values.waits.mode) {
        WaitMode::LightOff => {
            if locked.light_off_delay_s.is_none() {
                seconds_row(ui, &tr!("print-light-off"), &mut values.light_off_delay_s);
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

/// The one way of waiting the printer takes, where it takes only one.
fn locked_mode(locked: &PrinterTuning) -> Option<WaitMode> {
    locked
        .wait_mode
        .or_else(|| locked.waits.map(|waits| waits.mode))
}

/// Whether the printer leaves any of the waits to the resin.
fn shows_waits(locked: &PrinterTuning) -> bool {
    match locked_mode(locked) {
        None => true,
        Some(WaitMode::LightOff) => locked.light_off_delay_s.is_none(),
        Some(WaitMode::Rest) => locked.waits.is_none(),
    }
}

fn seconds_row(ui: &mut egui::Ui, label: &str, value: &mut f32) {
    number_row(ui, label, value, &tr!("unit-s"), FINE_STEP, 0.0..=60.0, 2);
}

/// Exposure: how long each layer is lit, and how hard. The exposure is measured at the
/// layer height above it and follows it, saying so until it is touched.
fn light(ui: &mut egui::Ui, resin: &mut ResinDraft, locked: &PrinterTuning) {
    let mut layer_height_mm = resin.draft.values.layer_height_mm;
    if locked.layer_height_mm.is_none()
        && number_row(
            ui,
            &tr!("resin-layer-height"),
            &mut layer_height_mm,
            &tr!("unit-mm"),
            FINE_STEP,
            MIN_LAYER_HEIGHT_MM..=MAX_LAYER_HEIGHT_MM,
            3,
        )
    {
        resin.set_layer_height(layer_height_mm);
    }
    let was = resin
        .carried()
        .map(|carried| (carried.exposure_s, carried.from_mm));
    let values = &mut resin.draft.values;
    if locked.exposure_s.is_some() {
        bottom_and_power(ui, values, locked);
        return;
    }
    let carried = carried_row(
        ui,
        &tr!("print-exposure"),
        &mut values.exposure_s,
        &tr!("unit-s"),
        FINE_STEP,
        0.05..=120.0,
        2,
        was.map(|(exposure_s, _)| exposure_s),
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
    bottom_and_power(ui, values, locked);
    match carried {
        Carried::Edited => resin.exposure_edited(),
        Carried::Reverted => resin.revert_exposure(),
        Carried::Untouched => {}
    }
}

/// The bottom block, the ramp out of it and the light's power, where the printer leaves
/// them to the resin.
fn bottom_and_power(ui: &mut egui::Ui, values: &mut MaterialProfile, locked: &PrinterTuning) {
    if locked.bottom_exposure_s.is_none() {
        number_row(
            ui,
            &tr!("print-bottom-exposure"),
            &mut values.bottom_exposure_s,
            &tr!("unit-s"),
            COARSE_STEP,
            0.05..=600.0,
            2,
        );
    }
    if locked.bottom_layers.is_none() {
        whole_row(
            ui,
            &tr!("print-bottom-layers"),
            &mut values.bottom_layers,
            1.0,
            1..=200,
        );
    }
    if locked.transition_layers.is_none() {
        transition_row(ui, values);
    }
    if locked.light_pwm.is_none() {
        pwm_row(ui, &tr!("resin-light-power"), &mut values.light_pwm);
    }
    if locked.bottom_light_pwm.is_none() {
        pwm_row(ui, &tr!("resin-bottom-power"), &mut values.bottom_light_pwm);
    }
}

/// Motion: how the plate peels the layer off the film and comes back. What the printer
/// locks is not offered.
fn motion(ui: &mut egui::Ui, values: &mut MaterialProfile, locked: &PrinterTuning) {
    let rows = [
        (
            tr!("print-lift"),
            locked.lift_distance_mm,
            &mut values.lift_distance_mm,
            Travel::Distance,
        ),
        (
            tr!("print-lift-speed"),
            locked.lift_speed_mm_min,
            &mut values.lift_speed_mm_min,
            Travel::Speed,
        ),
        (
            tr!("resin-retract"),
            locked.retract_distance_mm,
            &mut values.retract_distance_mm,
            Travel::Distance,
        ),
        (
            tr!("print-retract-speed"),
            locked.retract_speed_mm_min,
            &mut values.retract_speed_mm_min,
            Travel::Speed,
        ),
    ];
    travel_rows(ui, rows);
    ui.add_space(8.0);
    let bottom = [
        (
            tr!("resin-bottom-lift"),
            locked.bottom_lift_distance_mm,
            &mut values.bottom_lift_distance_mm,
            Travel::Distance,
        ),
        (
            tr!("resin-bottom-lift-speed"),
            locked.bottom_lift_speed_mm_min,
            &mut values.bottom_lift_speed_mm_min,
            Travel::Speed,
        ),
        (
            tr!("resin-bottom-retract-speed"),
            locked.bottom_retract_speed_mm_min,
            &mut values.bottom_retract_speed_mm_min,
            Travel::Speed,
        ),
    ];
    travel_rows(ui, bottom);
}

/// Whether a lift or retract is a distance or a speed, which sets its unit and its range.
#[derive(Clone, Copy)]
enum Travel {
    Distance,
    Speed,
}

/// The rows of a peel the printer leaves to the resin.
fn travel_rows<const N: usize>(
    ui: &mut egui::Ui,
    rows: [(String, Option<f32>, &mut f32, Travel); N],
) {
    for (label, locked, value, travel) in rows {
        if locked.is_some() {
            continue;
        }
        match travel {
            Travel::Distance => {
                number_row(
                    ui,
                    &label,
                    value,
                    &tr!("unit-mm"),
                    COARSE_STEP,
                    0.1..=100.0,
                    2,
                );
            }
            Travel::Speed => {
                number_row(ui, &label, value, &tr!("unit-mm-min"), 1.0, 1.0..=1000.0, 1);
            }
        };
    }
}

/// Whether the printer runs every part of the peel itself, which leaves the resin no motion.
fn locks_all_motion(locked: &PrinterTuning) -> bool {
    [
        locked.lift_distance_mm,
        locked.lift_speed_mm_min,
        locked.retract_distance_mm,
        locked.retract_speed_mm_min,
        locked.bottom_lift_distance_mm,
        locked.bottom_lift_speed_mm_min,
        locked.bottom_retract_speed_mm_min,
    ]
    .iter()
    .all(Option::is_some)
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
    if crate::ui::count_row(
        ui,
        &tr!("print-transition-layers"),
        &mut layers,
        "",
        1.0,
        0..=200,
    ) {
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
