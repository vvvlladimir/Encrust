use printer_profiles::{MaterialProfile, WaitMode};

use crate::slicing::Slicing;
use crate::state::Machine;
use crate::ui::{
    Carried, Segment, Segmented, carried_row, count_row, describe, hint, number_row, readings,
    section, subheading,
};

/// The resin's numbers for this print, edited where the print is looked at. They change the
/// copy the plate is sliced with, never the profile: a new printer or resin puts them back.
pub fn ui(ui: &mut egui::Ui, machine: &mut Machine) {
    let layer_height_mm = machine.slicing.layer_height_mm();
    let slicing = &mut machine.slicing;
    section(ui, "Print settings", Some("this session"), |ui| {
        describe(
            ui,
            "What this print is exposed and moved with. Edits change this session's slices \
             only; the resin profile keeps its own numbers.",
        );
        readings(ui, &[("Layer height", format!("{layer_height_mm:.3} mm"))]);

        exposure(ui, slicing);
        let material = &mut slicing.material;
        transition(ui, material);
        waiting(ui, material);
        motion(ui, material);
    });
}

/// The normal exposure follows the layer height, and says so until it is touched; see
/// `docs/decisions/0128`.
fn exposure(ui: &mut egui::Ui, slicing: &mut Slicing) {
    subheading(ui, "Exposure");
    let was = slicing.rescaled().map(|rescaled| rescaled.exposure_s);
    let material = &mut slicing.material;
    count_row(
        ui,
        "Bottom layers",
        &mut material.bottom_layers,
        "",
        0.2,
        0..=200,
    );
    let carried = carried_row(
        ui,
        "Exposure",
        &mut material.exposure_s,
        "s",
        0.01,
        0.1..=300.0,
        2,
        was,
    );
    number_row(
        ui,
        "Bottom exposure",
        &mut material.bottom_exposure_s,
        "s",
        0.1,
        0.1..=600.0,
        2,
    );
    match carried {
        Carried::Edited => slicing.exposure_edited(),
        Carried::Reverted => slicing.revert_exposure(),
        Carried::Untouched => {}
    }
    if let Some(rescaled) = slicing.rescaled() {
        hint(
            ui,
            &format!(
                "Carried from {:.2} s at {:.3} mm to the {:.3} mm layer.",
                rescaled.exposure_s,
                rescaled.from_mm,
                slicing.layer_height_mm()
            ),
        );
    }
}

fn transition(ui: &mut egui::Ui, material: &mut MaterialProfile) {
    subheading(ui, "Transition");
    let mut transition = u32::from(material.transition_layers);
    if count_row(ui, "Transition layers", &mut transition, "", 0.2, 0..=100) {
        material.transition_layers = transition.min(u32::from(u16::MAX)) as u16;
    }
    let step_s = (material.bottom_exposure_s - material.exposure_s)
        / (f32::from(material.transition_layers) + 1.0);
    readings(
        ui,
        &[
            ("Transition type", "Linear".to_owned()),
            ("Step between layers", format!("{step_s:.3} s")),
        ],
    );
}

/// What the printer waits for between moves: one light-off delay, or a rest at each stop.
fn waiting(ui: &mut egui::Ui, material: &mut MaterialProfile) {
    subheading(ui, "Waiting");
    let mut mode = material.waits.mode;
    let modes = [
        Segment::new(WaitMode::LightOff, "Light off"),
        Segment::new(WaitMode::Rest, "Resting"),
    ];
    if Segmented::new(&modes)
        .width(ui.available_width())
        .show(ui, &mut mode)
    {
        material.waits.mode = mode;
    }
    match material.waits.mode {
        WaitMode::LightOff => {
            number_row(
                ui,
                "Light off",
                &mut material.light_off_delay_s,
                "s",
                0.05,
                0.0..=60.0,
                2,
            );
        }
        WaitMode::Rest => {
            let waits = &mut material.waits;
            number_row(
                ui,
                "Before lift",
                &mut waits.before_lift_s,
                "s",
                0.05,
                0.0..=60.0,
                2,
            );
            number_row(
                ui,
                "After lift",
                &mut waits.after_lift_s,
                "s",
                0.05,
                0.0..=60.0,
                2,
            );
            number_row(
                ui,
                "After retract",
                &mut waits.after_retract_s,
                "s",
                0.05,
                0.0..=60.0,
                2,
            );
        }
    }
}

fn motion(ui: &mut egui::Ui, material: &mut MaterialProfile) {
    subheading(ui, "Motion");
    number_row(
        ui,
        "Lift",
        &mut material.lift_distance_mm,
        "mm",
        0.1,
        0.5..=30.0,
        2,
    );
    number_row(
        ui,
        "Lift speed",
        &mut material.lift_speed_mm_min,
        "mm/min",
        1.0,
        1.0..=1000.0,
        0,
    );
    number_row(
        ui,
        "Retract speed",
        &mut material.retract_speed_mm_min,
        "mm/min",
        1.0,
        1.0..=1000.0,
        0,
    );
}
