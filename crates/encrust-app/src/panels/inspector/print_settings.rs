use printer_profiles::{MaterialProfile, PrinterTuning, WaitMode};

use super::layers;
use crate::i18n::tr;
use crate::slicing::Slicing;
use crate::state::Machine;
use crate::ui::{
    Carried, Segment, Segmented, carried_row, count_row, describe, hint, number_row, readings,
    section, subheading,
};

/// How this plate is cut and exposed: a form rather than a tool, opened from the top bar.
/// The exposure, waits and motion change the copy the plate is sliced with, never the resin
/// profile: a new printer or resin puts them back.
pub fn ui(ui: &mut egui::Ui, machine: &mut Machine) {
    let locked = machine
        .slicing
        .printer
        .as_ref()
        .map(|printer| printer.locked.clone())
        .unwrap_or_default();
    let mut to_the_printer = layers::layers(ui, machine);
    section(
        ui,
        &tr!("print-exposure"),
        Some(&tr!("print-this-session")),
        |ui| {
            describe(ui, &tr!("print-exposure-help"));
            describe(ui, &tr!("print-from-resin"));
            exposure(ui, &mut machine.slicing, &locked);
            if locked.transition_layers.is_none() {
                transition(ui, &mut machine.slicing.material);
            }
        },
    );
    to_the_printer |= layers::bands(ui, machine);
    layers::edges(ui, machine);
    if !locked.locks_waiting() {
        section(ui, &tr!("print-waiting"), None, |ui| {
            describe(ui, &tr!("print-waiting-help"));
            waiting(ui, &mut machine.slicing.material, &locked)
        });
    }
    if [
        locked.lift_distance_mm,
        locked.lift_speed_mm_min,
        locked.retract_speed_mm_min,
    ]
    .iter()
    .any(Option::is_none)
    {
        section(ui, &tr!("print-motion"), None, |ui| {
            describe(ui, &tr!("print-motion-help"));
            motion(ui, &mut machine.slicing.material, &locked)
        });
    }
    if to_the_printer {
        let printer = machine.slicing.printer_id.clone();
        let resin = machine.slicing.resin_id.clone();
        machine.settings.open_machines(
            &machine.slicing.catalogue,
            printer.as_deref(),
            resin.as_deref(),
        );
    }
}

/// The normal exposure follows the layer height, and says so until it is touched; see
/// `docs/decisions/0128`.
fn exposure(ui: &mut egui::Ui, slicing: &mut Slicing, locked: &PrinterTuning) {
    let was = slicing.rescaled().map(|rescaled| rescaled.exposure_s);
    let material = &mut slicing.material;
    if locked.bottom_layers.is_none() {
        count_row(
            ui,
            &tr!("print-bottom-layers"),
            &mut material.bottom_layers,
            "",
            0.2,
            0..=200,
        );
    }
    let carried = if locked.exposure_s.is_none() {
        carried_row(
            ui,
            &tr!("print-exposure"),
            &mut material.exposure_s,
            &tr!("unit-s"),
            0.01,
            0.1..=300.0,
            2,
            was,
        )
    } else {
        Carried::Untouched
    };
    if locked.bottom_exposure_s.is_none() {
        number_row(
            ui,
            &tr!("print-bottom-exposure"),
            &mut material.bottom_exposure_s,
            &tr!("unit-s"),
            0.1,
            0.1..=600.0,
            2,
        );
    }
    match carried {
        Carried::Edited => slicing.exposure_edited(),
        Carried::Reverted => slicing.revert_exposure(),
        Carried::Untouched => {}
    }
    if let Some(rescaled) = slicing.rescaled() {
        hint(
            ui,
            &tr!(
                "print-carried",
                exposure = format!("{:.2}", rescaled.exposure_s),
                from = format!("{:.3}", rescaled.from_mm),
                to = format!("{:.3}", slicing.layer_height_mm())
            ),
        );
    }
}

fn transition(ui: &mut egui::Ui, material: &mut MaterialProfile) {
    subheading(ui, &tr!("print-transition"));
    let mut transition = u32::from(material.transition_layers);
    if count_row(
        ui,
        &tr!("print-transition-layers"),
        &mut transition,
        "",
        0.2,
        0..=100,
    ) {
        material.transition_layers = transition.min(u32::from(u16::MAX)) as u16;
    }
    let step_s = (material.bottom_exposure_s - material.exposure_s)
        / (f32::from(material.transition_layers) + 1.0);
    readings(
        ui,
        &[
            (tr!("print-transition-type"), tr!("print-linear")),
            (
                tr!("print-step"),
                tr!("value-s", value = format!("{step_s:.3}")),
            ),
        ],
    );
}

/// What the printer waits for between moves: one light-off delay, or a rest at each stop.
/// A printer that takes one mode only is shown in it, with no choice.
fn waiting(ui: &mut egui::Ui, material: &mut MaterialProfile, locked: &PrinterTuning) {
    if let Some(mode) = locked.held_wait_mode() {
        material.waits.mode = mode;
    } else {
        let mut mode = material.waits.mode;
        let modes = [
            Segment::new(WaitMode::LightOff, tr!("print-light-off")),
            Segment::new(WaitMode::Rest, tr!("print-resting")),
        ];
        if Segmented::new(&modes)
            .width(ui.available_width())
            .show(ui, &mut mode)
        {
            material.waits.mode = mode;
        }
    }
    match material.waits.mode {
        WaitMode::LightOff if locked.light_off_delay_s.is_none() => {
            number_row(
                ui,
                &tr!("print-light-off"),
                &mut material.light_off_delay_s,
                &tr!("unit-s"),
                0.05,
                0.0..=60.0,
                2,
            );
        }
        WaitMode::Rest if locked.waits.is_none() => {
            let waits = &mut material.waits;
            number_row(
                ui,
                &tr!("print-before-lift"),
                &mut waits.before_lift_s,
                &tr!("unit-s"),
                0.05,
                0.0..=60.0,
                2,
            );
            number_row(
                ui,
                &tr!("print-after-lift"),
                &mut waits.after_lift_s,
                &tr!("unit-s"),
                0.05,
                0.0..=60.0,
                2,
            );
            number_row(
                ui,
                &tr!("print-after-retract"),
                &mut waits.after_retract_s,
                &tr!("unit-s"),
                0.05,
                0.0..=60.0,
                2,
            );
        }
        _ => {}
    }
}

/// The peel, less what the printer locks.
fn motion(ui: &mut egui::Ui, material: &mut MaterialProfile, locked: &PrinterTuning) {
    if locked.lift_distance_mm.is_none() {
        number_row(
            ui,
            &tr!("print-lift"),
            &mut material.lift_distance_mm,
            &tr!("unit-mm"),
            0.1,
            0.5..=30.0,
            2,
        );
    }
    let speeds = [
        (
            tr!("print-lift-speed"),
            locked.lift_speed_mm_min,
            &mut material.lift_speed_mm_min,
        ),
        (
            tr!("print-retract-speed"),
            locked.retract_speed_mm_min,
            &mut material.retract_speed_mm_min,
        ),
    ];
    for (label, locked, value) in speeds {
        if locked.is_none() {
            number_row(ui, &label, value, &tr!("unit-mm-min"), 1.0, 1.0..=1000.0, 0);
        }
    }
}
