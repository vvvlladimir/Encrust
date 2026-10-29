use std::num::NonZeroU8;

use core_format::ExposureRange;
use core_slicer::{AdaptiveSettings, ONE_SAMPLE};
use printer_profiles::Limit;

use crate::i18n::tr;
use crate::panels::edge_fields;
use crate::slicing::{MAX_LAYER_HEIGHT_MM, MIN_LAYER_HEIGHT_MM};
use crate::state::Machine;
use crate::ui::{
    Carried, Held, carried_row, count_row, describe, hint, icon, limited_row, list, list_row,
    nested, number_row, secondary_button, section, subheading_with_switch, theme,
};

/// Layer heights are typed in microns; one is dragged a point.
const UM_PER_MM: f32 = 1000.0;

/// Millimetres and seconds per point of drag on an exposure band.
const BAND_STEP: f64 = 0.1;
const EXPOSURE_STEP: f64 = 0.05;

/// Height a new band covers when there is nothing to hang it off: the first ten
/// millimetres, which is where a bottom-heavy model needs the extra exposure.
const NEW_BAND_MM: f32 = 10.0;

/// The way to the printer's settings, offered under the first thing it refuses and no other.
#[derive(Default)]
pub struct PrinterWay {
    offered: bool,
    /// The way was taken this frame.
    pub taken: bool,
}

/// How thick a layer is, how many planes it samples, and whether its height follows the
/// surface.
pub fn layers(ui: &mut egui::Ui, machine: &mut Machine, way: &mut PrinterWay) {
    let slicing = &mut machine.slicing;
    section(ui, &tr!("summary-layers"), None, |ui| {
        describe(ui, &tr!("layers-height-help"));
        let ceiling = slicing.adaptive.is_some();
        let mut layer_height_um = slicing.layer_height_mm() * UM_PER_MM;
        let label = if ceiling {
            tr!("layers-thickest")
        } else {
            tr!("resin-layer-height")
        };
        let limit = slicing
            .printer
            .as_ref()
            .map(|printer| microns(&printer.limits.layer_height_mm))
            .unwrap_or_default();
        let held = Held::layer_height(&limit);
        if limited_row(
            ui,
            &label,
            &mut layer_height_um,
            &tr!("unit-um"),
            1.0,
            MIN_LAYER_HEIGHT_MM * UM_PER_MM..=MAX_LAYER_HEIGHT_MM * UM_PER_MM,
            Some(0),
            held,
        ) {
            slicing.set_layer_height(layer_height_um / UM_PER_MM);
        }
        exposure_note(ui, slicing);
        samples_row(ui, &mut slicing.samples);
        adaptive_rows(ui, slicing, way);
    });
}

/// A limit stated in millimetres, in microns.
fn microns(limit: &Limit) -> Limit {
    let um = |mm: Option<f32>| mm.map(|mm| mm * UM_PER_MM);
    Limit {
        min: um(limit.min),
        max: um(limit.max),
        warn_min: um(limit.warn_min),
        warn_max: um(limit.warn_max),
    }
}

/// The bands of height that take an exposure of their own.
pub fn bands(ui: &mut egui::Ui, machine: &mut Machine, way: &mut PrinterWay) {
    let count = machine.slicing.exposure.len();
    let aside = (count > 0).then(|| tr!("layers-bands", count = count));
    section(ui, &tr!("layers-by-height"), aside.as_deref(), |ui| {
        exposure_bands(ui, &mut machine.slicing, way);
    });
}

/// The greys a mask's edge is rounded with, to hide the steps of the pixels.
pub fn edges(ui: &mut egui::Ui, machine: &mut Machine) {
    let edges = &mut machine.slicing.edges;
    let aside = edge_fields::summary(edges);
    section(ui, &tr!("layers-edge-smoothing"), Some(&aside), |ui| {
        describe(ui, &tr!("layers-edge-note"));
        edge_fields::rows(ui, edges);
    });
}

/// Why a setting here cannot be used on the printer in hand, and, the first time on the
/// panel, the way to the firmware switch that would say otherwise.
fn firmware_note(ui: &mut egui::Ui, why: &str, way: &mut PrinterWay) {
    hint(ui, why);
    if way.offered {
        return;
    }
    way.offered = true;
    ui.add_space(theme::NOTE_GAP);
    way.taken |= secondary_button(ui, icon::SETTINGS, &tr!("layers-open-printer")).clicked();
}

/// How many planes are sampled inside a layer. More finds a feature thinner than the
/// layer, and costs a slicing pass each.
fn samples_row(ui: &mut egui::Ui, samples: &mut NonZeroU8) {
    let mut value = u32::from(samples.get());
    if count_row(ui, &tr!("layers-planes"), &mut value, "", 1.0, 1..=5) {
        *samples = NonZeroU8::new(value as u8).unwrap_or(ONE_SAMPLE);
    }
    if *samples != ONE_SAMPLE {
        describe(ui, &tr!("layers-planes-note"));
    }
}

/// What a normal layer is exposed for when an adaptive stack cannot reach the thickest
/// layer asked for: every thickness is a whole number of the thinnest, see ADR 0091.
fn exposure_note(ui: &mut egui::Ui, slicing: &crate::slicing::Slicing) {
    let material = &slicing.material;
    let Some(adaptive) = slicing.adaptive else {
        return;
    };
    let thickest = AdaptiveSettings {
        max_height_mm: slicing.layer_height_mm(),
        ..adaptive
    }
    .reachable_max_mm();
    if (thickest - material.layer_height_mm).abs() < 1e-4 {
        return;
    }
    let exposure_s = material.exposure_at(material.exposure_s, thickest);
    describe(
        ui,
        &tr!(
            "layers-thickest-reached",
            thickest = format!("{:.0}", thickest * UM_PER_MM),
            asked = format!("{:.0}", material.layer_height_mm * UM_PER_MM),
            exposure = format!("{exposure_s:.2}"),
            resin = format!("{:.2}", material.exposure_s)
        ),
    );
}

/// Slicing thick where the surface stands up and thin where it lies down, and what that
/// is allowed to cost.
fn adaptive_rows(ui: &mut egui::Ui, slicing: &mut crate::slicing::Slicing, way: &mut PrinterWay) {
    // Nothing is known of a machine not picked yet, so nothing is refused for it either.
    let refused = slicing.printer.is_some() && !slicing.printer_reads_variable_height();
    let mut on = slicing.adaptive.is_some();
    let flipped = ui
        .add_enabled_ui(!refused, |ui| {
            subheading_with_switch(ui, &tr!("layers-adaptive"), &mut on)
        })
        .inner;
    if flipped {
        slicing.adaptive = on.then(AdaptiveSettings::default);
    }
    if refused {
        firmware_note(ui, &tr!("layers-adaptive-refused"), way);
    }
    let Some(adaptive) = slicing.adaptive.as_mut() else {
        return;
    };
    nested(ui, |ui| {
        microns_row(ui, &tr!("layers-cusp"), &mut adaptive.cusp_mm);
        microns_row(ui, &tr!("layers-thinnest"), &mut adaptive.min_height_mm);
    });
}

/// A height kept in millimetres, typed in microns within the layer heights the window takes.
fn microns_row(ui: &mut egui::Ui, label: &str, height_mm: &mut f32) {
    let mut um = *height_mm * UM_PER_MM;
    let range = MIN_LAYER_HEIGHT_MM * UM_PER_MM..=MAX_LAYER_HEIGHT_MM * UM_PER_MM;
    if number_row(ui, label, &mut um, &tr!("unit-um"), 1.0, range, 0) {
        *height_mm = um / UM_PER_MM;
    }
}

/// The bands of print height that take an exposure of their own. A band's exposure is
/// carried with the layer height like the resin's, and says so the same way.
fn exposure_bands(ui: &mut egui::Ui, slicing: &mut crate::slicing::Slicing, way: &mut PrinterWay) {
    let was: Vec<Option<f32>> = (0..slicing.exposure.len())
        .map(|index| {
            slicing
                .rescaled()
                .and_then(|rescaled| rescaled.bands.get(index))
                .map(|band| band.exposure_s)
        })
        .collect();
    let mut remove = None;
    let mut carried = Carried::Untouched;
    for (index, band) in slicing.exposure.iter_mut().enumerate() {
        if band_rows(ui, index, band, was[index], &mut carried) {
            remove = Some(index);
        }
    }
    match carried {
        Carried::Edited => slicing.exposure_edited(),
        Carried::Reverted => slicing.revert_exposure(),
        Carried::Untouched => {}
    }
    if let Some(index) = remove {
        slicing.exposure.remove(index);
        slicing.exposure_edited();
    }
    let bands = &slicing.exposure;

    if bands.is_empty() {
        describe(ui, &tr!("layers-no-bands"));
    } else {
        describe(ui, &tr!("layers-bands-note"));
    }
    ui.add_space(theme::NOTE_GAP);

    if slicing.printer.is_some() && !slicing.printer_reads_per_layer() {
        firmware_note(ui, &tr!("layers-bands-refused"), way);
        return;
    }
    if secondary_button(ui, icon::ADD, &tr!("layers-add-band")).clicked() {
        let from_mm = bands.last().map_or(0.0, |last| last.to_mm);
        let resin_exposure_s = slicing.material.exposure_s;
        slicing.exposure.push(ExposureRange::new(
            from_mm,
            from_mm + NEW_BAND_MM,
            resin_exposure_s,
        ));
        slicing.exposure_edited();
    }
}

/// One band's fields. Answers whether its remove button was pressed.
fn band_rows(
    ui: &mut egui::Ui,
    index: usize,
    band: &mut ExposureRange,
    was: Option<f32>,
    carried: &mut Carried,
) -> bool {
    let name = tr!("layers-band", index = index + 1);
    let remove_tip = tr!("layers-remove-band");
    let tint = Some(theme::band_tint(index));
    let remove = list(ui, |ui| {
        let drop = Some((icon::REMOVE, remove_tip.as_str()));
        list_row(ui, &name, tint, false, drop, None, None, None).removed
    });
    number_row(
        ui,
        &tr!("layers-from"),
        &mut band.from_mm,
        &tr!("unit-mm"),
        BAND_STEP,
        0.0..=1000.0,
        2,
    );
    number_row(
        ui,
        &tr!("layers-to"),
        &mut band.to_mm,
        &tr!("unit-mm"),
        BAND_STEP,
        0.0..=1000.0,
        2,
    );
    match carried_row(
        ui,
        &tr!("print-exposure"),
        &mut band.exposure_s,
        &tr!("unit-s"),
        EXPOSURE_STEP,
        0.01..=200.0,
        2,
        was,
    ) {
        Carried::Untouched => {}
        action => *carried = action,
    }
    ui.add_space(theme::NOTE_GAP);
    remove
}
