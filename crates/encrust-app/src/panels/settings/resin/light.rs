//! The light rows of a resin: layer height, exposures, bottom layers, power and transition.

use super::{COARSE_STEP, FINE_STEP};
use crate::i18n::tr;
use crate::settings::ResinDraft;
use crate::slicing::{MAX_LAYER_HEIGHT_MM, MIN_LAYER_HEIGHT_MM};
use crate::ui::{Carried, Held, carried_row, hint, limit_verdict, limited_row};
use printer_profiles::{Limit, MaterialProfile, PrinterLimits, PrinterTuning};

/// Exposure: how long each layer is lit, and how hard. The exposure is measured at the
/// layer height above it and follows it, saying so until it is touched.
pub(super) fn light(
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

pub(super) fn layer_height_row(ui: &mut egui::Ui, resin: &mut ResinDraft, limit: &Limit) {
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
pub(super) fn exposure_row(
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
pub(super) fn bottom_and_power(
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
pub(super) fn transition_row(ui: &mut egui::Ui, values: &mut MaterialProfile, held: Held<'_>) {
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
pub(super) fn pwm_row(ui: &mut egui::Ui, label: &str, value: &mut u8) {
    let mut power = u32::from(*value);
    if crate::ui::count_row(ui, label, &mut power, "", 1.0, 0..=255) {
        *value = power as u8;
    }
}
