use std::num::NonZeroU8;

use core_format::ExposureRange;
use core_slicer::{AdaptiveSettings, ONE_SAMPLE};

use crate::slicing::{MAX_LAYER_HEIGHT_MM, MIN_LAYER_HEIGHT_MM};
use crate::state::Machine;
use crate::ui::{
    Carried, Segment, Segmented, carried_row, describe, hint, icon, icon_button, later, nested,
    number_row, secondary_button, section, subheading, switch, theme, tone,
};

/// Eight greys is what other slicers call anti-aliasing level 8, and the most a panel
/// holds; see `docs/decisions/0113`.
const DEFAULT_GREY_LEVELS: NonZeroU8 = NonZeroU8::new(8).expect("eight is not zero");

/// Millimetres per point of drag on the layer height field: a micron a point.
const LAYER_HEIGHT_STEP: f64 = 0.001;

/// Millimetres and seconds per point of drag on an exposure band.
const BAND_STEP: f64 = 0.1;
const EXPOSURE_STEP: f64 = 0.05;

/// Height a new band covers when there is nothing to hang it off: the first ten
/// millimetres, which is where a bottom-heavy model needs the extra exposure.
const NEW_BAND_MM: f32 = 10.0;

/// How thick a layer is, how many planes it samples, and whether its height follows the
/// surface. Answers whether the way to the printer's settings was taken.
pub fn layers(ui: &mut egui::Ui, machine: &mut Machine) -> bool {
    let slicing = &mut machine.slicing;
    section(ui, "Layers", None, |ui| {
        let ceiling = slicing.adaptive.is_some();
        let mut layer_height_mm = slicing.layer_height_mm();
        let label = if ceiling {
            "Thickest layer"
        } else {
            "Layer height"
        };
        if number_row(
            ui,
            label,
            &mut layer_height_mm,
            "mm",
            LAYER_HEIGHT_STEP,
            MIN_LAYER_HEIGHT_MM..=MAX_LAYER_HEIGHT_MM,
            3,
        ) {
            slicing.set_layer_height(layer_height_mm);
        }
        carried_note(ui, slicing);
        exposure_note(ui, slicing);
        samples_row(ui, &mut slicing.samples);

        subheading(ui, "Adaptive height");
        adaptive_rows(ui, slicing)
    })
    .unwrap_or_default()
}

/// The bands of height that take an exposure of their own. Answers whether the way to the
/// printer's settings was taken.
pub fn bands(ui: &mut egui::Ui, machine: &mut Machine) -> bool {
    let count = machine.slicing.exposure.len();
    let aside = (count > 0).then(|| format!("{count} band(s)"));
    section(ui, "By height", aside.as_deref(), |ui| {
        exposure_bands(ui, &mut machine.slicing)
    })
    .unwrap_or_default()
}

/// The greys a mask's edge is rounded with, to hide the steps of the pixels.
pub fn edges(ui: &mut egui::Ui, machine: &mut Machine) {
    let slicing = &mut machine.slicing;
    section(ui, "Edge smoothing", None, |ui| {
        describe(
            ui,
            "Grey levels on the mask edge, to hide the pixel stairs.",
        );
        // TODO(step-8): choose the smoothing by its look, from named presets, as well as
        // by its numbers.
        later(ui, |ui| {
            let mut by_look = false;
            let choices = [Segment::new(true, "Look"), Segment::new(false, "Number")];
            Segmented::new(&choices)
                .width(ui.available_width())
                .show(ui, &mut by_look);
        });
        switch(ui, &mut slicing.anti_alias, "Anti-aliased masks");
        if slicing.anti_alias {
            nested(ui, |ui| {
                grey_levels(ui, &mut slicing.grey_levels);
                blur_row(ui, &mut slicing.blur_px);
            });
        }
    });
}

/// Why a setting here cannot be used on the printer in hand, and the way to the firmware
/// switch that would say otherwise. Answers whether that way was taken.
fn firmware_note(ui: &mut egui::Ui, why: &str) -> bool {
    hint(ui, why);
    ui.add_space(4.0);
    secondary_button(ui, icon::SETTINGS, "Open printer settings").clicked()
}

/// How many planes are sampled inside a layer. More finds a feature thinner than the
/// layer, and costs a slicing pass each.
fn samples_row(ui: &mut egui::Ui, samples: &mut NonZeroU8) {
    let mut value = f32::from(samples.get());
    if number_row(ui, "Planes a layer", &mut value, "", 1.0, 1.0..=5.0, 0) {
        *samples = NonZeroU8::new(value as u8).unwrap_or(ONE_SAMPLE);
    }
    if *samples != ONE_SAMPLE {
        describe(
            ui,
            "a feature thinner than a layer survives, at that many times the cut",
        );
    }
}

/// How many greys an edge is rounded to. Off is every grey the panel will take.
fn grey_levels(ui: &mut egui::Ui, levels: &mut Option<NonZeroU8>) {
    let mut limited = levels.is_some();
    if switch(ui, &mut limited, "Round edge grey").changed() {
        *levels = limited.then_some(DEFAULT_GREY_LEVELS);
    }
    if let Some(count) = levels.as_mut() {
        nested(ui, |ui| {
            let mut value = f32::from(count.get());
            if number_row(ui, "Grey levels", &mut value, "", 1.0, 2.0..=16.0, 0) {
                *count = NonZeroU8::new(value as u8).unwrap_or(DEFAULT_GREY_LEVELS);
            }
        });
    }
}

/// How many pixels either side an edge is faded over. Zero keeps it sharp.
fn blur_row(ui: &mut egui::Ui, blur_px: &mut u8) {
    let mut value = f32::from(*blur_px);
    if number_row(ui, "Edge blur", &mut value, "px", 1.0, 0.0..=8.0, 0) {
        *blur_px = value as u8;
    }
    if *blur_px > 2 {
        describe(
            ui,
            "a wide blur dims the wall inside the edge as well; one or two pixels is usual",
        );
    }
}

/// Says out loud that changing the layer height carried the exposure along, and offers
/// it back; see `docs/decisions/0128`.
fn carried_note(ui: &mut egui::Ui, slicing: &mut crate::slicing::Slicing) {
    let Some(rescaled) = slicing.rescaled() else {
        return;
    };
    let text = format!(
        "Exposure carried with the layer: {:.2} s at {:.3} mm is {:.2} s now.",
        rescaled.exposure_s, rescaled.from_mm, slicing.material.exposure_s
    );
    let back = format!("Back to {:.2} s", rescaled.exposure_s);
    let mut revert = false;
    // The button takes its width first, and the sentence wraps into what is left: a label
    // in a horizontal layout does not wrap, and a long one pushes the panel out.
    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
        revert = icon_button(ui, icon::RESET, &back).clicked();
        ui.add(
            egui::Label::new(
                egui::RichText::new(&text)
                    .font(theme::small())
                    .color(theme::colors().warn),
            )
            .wrap(),
        );
    });
    if revert {
        slicing.revert_exposure();
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
        &format!(
            "The thickest layer reached is {thickest:.3} mm, not {:.3} mm, so a normal layer \
             takes {exposure_s:.2} s rather than {:.2} s.",
            material.layer_height_mm, material.exposure_s
        ),
    );
}

/// Slicing thick where the surface stands up and thin where it lies down, and what that
/// is allowed to cost.
fn adaptive_rows(ui: &mut egui::Ui, slicing: &mut crate::slicing::Slicing) -> bool {
    // Nothing is known of a machine not picked yet, so nothing is refused for it either.
    let refused = slicing.printer.is_some() && !slicing.printer_reads_variable_height();
    let mut on = slicing.adaptive.is_some();
    let flipped = ui
        .add_enabled_ui(!refused, |ui| {
            switch(ui, &mut on, "Against a cusp target").changed()
        })
        .inner;
    if flipped {
        slicing.adaptive = on.then(AdaptiveSettings::default);
    }
    let mut to_the_printer = false;
    if refused {
        to_the_printer = firmware_note(
            ui,
            "This printer steps the plate by the header's height, so no layer can have one \
             of its own.",
        );
    }
    let Some(adaptive) = slicing.adaptive.as_mut() else {
        return to_the_printer;
    };
    nested(ui, |ui| {
        number_row(
            ui,
            "Cusp",
            &mut adaptive.cusp_mm,
            "mm",
            LAYER_HEIGHT_STEP,
            MIN_LAYER_HEIGHT_MM..=MAX_LAYER_HEIGHT_MM,
            3,
        );
        number_row(
            ui,
            "Thinnest layer",
            &mut adaptive.min_height_mm,
            "mm",
            LAYER_HEIGHT_STEP,
            MIN_LAYER_HEIGHT_MM..=MAX_LAYER_HEIGHT_MM,
            3,
        );
    });
    to_the_printer
}

/// The bands of print height that take an exposure of their own. A band's exposure is
/// carried with the layer height like the resin's, and says so the same way.
fn exposure_bands(ui: &mut egui::Ui, slicing: &mut crate::slicing::Slicing) -> bool {
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
        describe(
            ui,
            "Every layer takes the resin's exposure. A band replaces it over a height.",
        );
    } else {
        describe(
            ui,
            "The bottom block keeps the resin's own exposure whatever a band says.",
        );
    }
    ui.add_space(4.0);

    if slicing.printer.is_some() && !slicing.printer_reads_per_layer() {
        return firmware_note(
            ui,
            "This printer reads the header alone, so exposure cannot vary by height.",
        );
    }
    if secondary_button(ui, icon::ADD, "Add a band").clicked() {
        let from_mm = bands.last().map_or(0.0, |last| last.to_mm);
        let resin_exposure_s = slicing.material.exposure_s;
        slicing.exposure.push(ExposureRange::new(
            from_mm,
            from_mm + NEW_BAND_MM,
            resin_exposure_s,
        ));
        slicing.exposure_edited();
    }
    false
}

/// One band's fields. Answers whether its remove button was pressed.
fn band_rows(
    ui: &mut egui::Ui,
    index: usize,
    band: &mut ExposureRange,
    was: Option<f32>,
    carried: &mut Carried,
) -> bool {
    let mut remove = false;
    ui.horizontal(|ui| {
        chip(ui, theme::band_tint(index));
        tone(ui, &format!("Band {}", index + 1), theme::colors().text_mid);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            remove = icon_button(ui, icon::REMOVE, "Remove this band").clicked();
        });
    });
    number_row(
        ui,
        "From",
        &mut band.from_mm,
        "mm",
        BAND_STEP,
        0.0..=1000.0,
        2,
    );
    number_row(ui, "To", &mut band.to_mm, "mm", BAND_STEP, 0.0..=1000.0, 2);
    match carried_row(
        ui,
        "Exposure",
        &mut band.exposure_s,
        "s",
        EXPOSURE_STEP,
        0.01..=200.0,
        2,
        was,
    ) {
        Carried::Untouched => {}
        action => *carried = action,
    }
    ui.add_space(4.0);
    remove
}

/// The square of colour a band is washed over the model in, drawn beside its fields so
/// the two can be told apart at a glance.
fn chip(ui: &mut egui::Ui, tint: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 2.0, tint);
}
