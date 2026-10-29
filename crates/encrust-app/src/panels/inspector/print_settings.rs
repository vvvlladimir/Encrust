use printer_profiles::{Limit, MaterialProfile, PrinterLimits, PrinterTuning, WaitMode};

use super::layers;
use crate::i18n::tr;
use crate::slicing::Slicing;
use crate::state::Machine;
use crate::ui::{
    Carried, Held, Segment, Segmented, carried_row, describe, hint, limit_verdict, limited_row,
    number_row, readings, section, subheading,
};

/// How this plate is cut and exposed: a form opened from the top bar. Its edits change the
/// plate's copy, never the resin profile; a new printer or resin resets them.
pub fn ui(ui: &mut egui::Ui, machine: &mut Machine) {
    let (locked, limits, binary_only) = machine
        .slicing
        .printer
        .as_ref()
        .map(|printer| {
            let binary_only = printer.display.binary_only;
            (printer.locked.clone(), printer.limits.clone(), binary_only)
        })
        .unwrap_or_default();
    let mut to_the_printer = layers::layers(ui, machine);
    section(
        ui,
        &tr!("print-exposure"),
        Some(&tr!("print-this-session")),
        |ui| {
            describe(ui, &tr!("print-exposure-help"));
            describe(ui, &tr!("print-from-resin"));
            exposure(ui, &mut machine.slicing, &locked, &limits);
            if locked.transition_layers.is_none() {
                transition(ui, &mut machine.slicing.material, &limits.transition_layers);
            }
        },
    );
    to_the_printer |= layers::bands(ui, machine);
    if !binary_only {
        layers::edges(ui, machine);
    }
    if !locked.locks_waiting() {
        section(ui, &tr!("print-waiting"), None, |ui| {
            describe(ui, &tr!("print-waiting-help"));
            waiting(ui, &mut machine.slicing.material, &locked, &limits)
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
            motion(ui, &mut machine.slicing.material, &locked, &limits)
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
fn exposure(
    ui: &mut egui::Ui,
    slicing: &mut Slicing,
    locked: &PrinterTuning,
    limits: &PrinterLimits,
) {
    let was = slicing.rescaled().map(|rescaled| rescaled.exposure_s);
    let material = &mut slicing.material;
    if locked.bottom_layers.is_none() {
        bottom_layers_row(ui, material, &limits.bottom_layers);
    }
    let carried = if locked.exposure_s.is_none() {
        exposure_row(ui, material, &limits.exposure_s, was)
    } else {
        Carried::Untouched
    };
    if locked.bottom_exposure_s.is_none() {
        limited_row(
            ui,
            &tr!("print-bottom-exposure"),
            &mut material.bottom_exposure_s,
            &tr!("unit-s"),
            0.1,
            0.1..=600.0,
            Some(2),
            Held::unexplained(&limits.bottom_exposure_s),
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

fn bottom_layers_row(ui: &mut egui::Ui, material: &mut MaterialProfile, limit: &Limit) {
    let label = tr!("print-bottom-layers");
    let held = Held::bottom_layers(limit);
    limited_row(
        ui,
        &label,
        &mut material.bottom_layers,
        "",
        0.2,
        0..=200,
        None,
        held,
    );
}

/// The normal exposure, carried from `was` when the layer height moved, and its verdict.
fn exposure_row(
    ui: &mut egui::Ui,
    material: &mut MaterialProfile,
    limit: &Limit,
    was: Option<f32>,
) -> Carried {
    let unit = tr!("unit-s");
    let carried = carried_row(
        ui,
        &tr!("print-exposure"),
        &mut material.exposure_s,
        &unit,
        0.01,
        limit.range(0.1..=300.0),
        2,
        was,
    );
    limit_verdict(
        ui,
        Held::unexplained(limit),
        material.exposure_s,
        &unit,
        Some(2),
    );
    carried
}

fn transition(ui: &mut egui::Ui, material: &mut MaterialProfile, limit: &Limit) {
    subheading(ui, &tr!("print-transition"));
    let mut transition = u32::from(material.transition_layers);
    let label = tr!("print-transition-layers");
    if limited_row(
        ui,
        &label,
        &mut transition,
        "",
        0.2,
        0..=100,
        None,
        Held::unexplained(limit),
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
fn waiting(
    ui: &mut egui::Ui,
    material: &mut MaterialProfile,
    locked: &PrinterTuning,
    limits: &PrinterLimits,
) {
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
            limited_row(
                ui,
                &tr!("print-light-off"),
                &mut material.light_off_delay_s,
                &tr!("unit-s"),
                0.05,
                0.0..=60.0,
                Some(2),
                Held::unexplained(&limits.light_off_delay_s),
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
fn motion(
    ui: &mut egui::Ui,
    material: &mut MaterialProfile,
    locked: &PrinterTuning,
    limits: &PrinterLimits,
) {
    if locked.lift_distance_mm.is_none() {
        limited_row(
            ui,
            &tr!("print-lift"),
            &mut material.lift_distance_mm,
            &tr!("unit-mm"),
            0.1,
            0.5..=30.0,
            Some(2),
            Held::unexplained(&limits.lift_distance_mm),
        );
    }
    let speeds = [
        (
            tr!("print-lift-speed"),
            locked.lift_speed_mm_min,
            &mut material.lift_speed_mm_min,
            &limits.lift_speed_mm_min,
        ),
        (
            tr!("print-retract-speed"),
            locked.retract_speed_mm_min,
            &mut material.retract_speed_mm_min,
            &limits.retract_speed_mm_min,
        ),
    ];
    let unit = tr!("unit-mm-min");
    for (label, locked, value, limit) in speeds {
        if locked.is_none() {
            limited_row(
                ui,
                &label,
                value,
                &unit,
                1.0,
                1.0..=1000.0,
                Some(1),
                Held::unexplained(limit),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use egui_kittest::kittest::Queryable as _;

    use super::*;
    use crate::ui::theme;

    /// The form for the shipped printer `id`, after the theme has been put on.
    fn form_for(id: &str) -> egui_kittest::Harness<'static, Machine> {
        let mut machine = Machine::default();
        let printer = machine
            .slicing
            .catalogue
            .printer(id)
            .expect("a shipped machine")
            .profile
            .clone();
        machine.slicing.set_printer(printer, None);
        let mut dressed = false;
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(360.0, 2000.0))
            .build_ui_state(
                move |ui, machine: &mut Machine| {
                    if dressed {
                        super::ui(ui, machine);
                    } else {
                        theme::apply(ui.ctx());
                        dressed = true;
                    }
                },
                machine,
            );
        harness.run_steps(3);
        harness
    }

    #[test]
    fn a_binary_only_machine_offers_no_edge_smoothing() {
        let shown = |id| {
            form_for(id)
                .query_by_label_contains("Edge smoothing")
                .is_some()
        };
        assert!(shown("elegoo-saturn-4-ultra"));
        assert!(!shown("creality-halot-x1"), "a binary-only machine");
    }
}
