use std::num::NonZeroU8;

use core_format::ExposureRange;
use core_slicer::{AdaptiveSettings, ONE_SAMPLE};

use crate::i18n::tr;
use crate::slicing::{MAX_LAYER_HEIGHT_MM, MIN_LAYER_HEIGHT_MM};
use crate::state::Machine;
use crate::ui::{
    Carried, Segment, Segmented, carried_row, describe, hint, icon, icon_button, nested,
    number_row, secondary_button, section, subheading_with_switch, switch, theme, tone,
};

/// Eight greys is what other slicers call anti-aliasing level 8, and the most a panel
/// holds; see `docs/decisions/0113`.
const DEFAULT_GREY_LEVELS: NonZeroU8 = NonZeroU8::new(8).expect("eight is not zero");

/// The greys a sharp edge is rounded to: few enough that small lettering keeps its corners.
const SHARP_GREY_LEVELS: u8 = 4;
/// The blur a soft edge is faded over, pixels: past two it dims the wall inside the edge.
const SOFT_BLUR_PX: u8 = 2;

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
    section(ui, &tr!("summary-layers"), None, |ui| {
        describe(ui, &tr!("layers-height-help"));
        let ceiling = slicing.adaptive.is_some();
        let mut layer_height_mm = slicing.layer_height_mm();
        let label = if ceiling {
            tr!("layers-thickest")
        } else {
            tr!("resin-layer-height")
        };
        if number_row(
            ui,
            &label,
            &mut layer_height_mm,
            &tr!("unit-mm"),
            LAYER_HEIGHT_STEP,
            MIN_LAYER_HEIGHT_MM..=MAX_LAYER_HEIGHT_MM,
            3,
        ) {
            slicing.set_layer_height(layer_height_mm);
        }
        carried_note(ui, slicing);
        exposure_note(ui, slicing);
        samples_row(ui, &mut slicing.samples);

        adaptive_rows(ui, slicing)
    })
    .unwrap_or_default()
}

/// The bands of height that take an exposure of their own. Answers whether the way to the
/// printer's settings was taken.
pub fn bands(ui: &mut egui::Ui, machine: &mut Machine) -> bool {
    let count = machine.slicing.exposure.len();
    let aside = (count > 0).then(|| tr!("layers-bands", count = count));
    section(ui, &tr!("layers-by-height"), aside.as_deref(), |ui| {
        exposure_bands(ui, &mut machine.slicing)
    })
    .unwrap_or_default()
}

/// The greys a mask's edge is rounded with, to hide the steps of the pixels: one of four
/// looks, or the numbers behind them.
pub fn edges(ui: &mut egui::Ui, machine: &mut Machine) {
    let slicing = &mut machine.slicing;
    let look = Look::of(slicing.anti_alias, slicing.grey_levels, slicing.blur_px);
    let aside = slicing
        .grey_levels
        .map(|levels| tr!("layers-levels", count = levels.get()));
    section(ui, &tr!("layers-edge-smoothing"), aside.as_deref(), |ui| {
        describe(ui, &tr!("layers-edge-note"));
        let by_look_id = ui.id().with("edges-by-look");
        let mut by_look = ui.data(|data| data.get_temp(by_look_id).unwrap_or(look.is_some()));
        let choices = [
            Segment::new(true, tr!("layers-look")),
            Segment::new(false, tr!("layers-number")),
        ];
        Segmented::new(&choices)
            .width(ui.available_width())
            .show(ui, &mut by_look);
        ui.data_mut(|data| data.insert_temp(by_look_id, by_look));

        if by_look {
            if let Some(picked) = looks(ui, look) {
                picked.apply(slicing);
            }
            return;
        }
        switch(ui, &mut slicing.anti_alias, &tr!("layers-anti-aliased"));
        if slicing.anti_alias {
            nested(ui, |ui| {
                grey_levels(ui, &mut slicing.grey_levels);
                blur_row(ui, &mut slicing.blur_px);
            });
        }
    });
}

/// A named way for an edge to come out, standing for the numbers that make it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Look {
    Off,
    Sharp,
    Balanced,
    Soft,
}

impl Look {
    const ALL: [Self; 4] = [Self::Off, Self::Sharp, Self::Balanced, Self::Soft];

    fn name(self) -> String {
        match self {
            Self::Off => tr!("look-off"),
            Self::Sharp => tr!("look-sharp"),
            Self::Balanced => tr!("look-balanced"),
            Self::Soft => tr!("look-soft"),
        }
    }

    fn line(self) -> String {
        match self {
            Self::Off => tr!("look-off-line"),
            Self::Sharp => tr!("look-sharp-line"),
            Self::Balanced => tr!("look-balanced-line"),
            Self::Soft => tr!("look-soft-line"),
        }
    }

    /// The greys an edge is rounded to and its blur in pixels, or `None` for no smoothing,
    /// which leaves the numbers as they were for the next time it is turned on.
    fn settings(self) -> Option<(Option<NonZeroU8>, u8)> {
        match self {
            Self::Off => None,
            Self::Sharp => Some((NonZeroU8::new(SHARP_GREY_LEVELS), 0)),
            Self::Balanced => Some((None, 0)),
            Self::Soft => Some((None, SOFT_BLUR_PX)),
        }
    }

    fn apply(self, slicing: &mut crate::slicing::Slicing) {
        slicing.anti_alias = self.settings().is_some();
        if let Some((grey_levels, blur_px)) = self.settings() {
            (slicing.grey_levels, slicing.blur_px) = (grey_levels, blur_px);
        }
    }

    /// The look these numbers make, or `None` for numbers typed by hand.
    fn of(anti_alias: bool, grey_levels: Option<NonZeroU8>, blur_px: u8) -> Option<Self> {
        if !anti_alias {
            return Some(Self::Off);
        }
        Self::ALL
            .into_iter()
            .find(|look| look.settings() == Some((grey_levels, blur_px)))
    }
}

/// The four looks as a column of choices, the one in use lit. Answers the one pressed.
fn looks(ui: &mut egui::Ui, current: Option<Look>) -> Option<Look> {
    let colors = theme::colors();
    let mut picked = None;
    for look in Look::ALL {
        let on = current == Some(look);
        let name = egui::RichText::new(look.name())
            .font(theme::label())
            .color(if on {
                colors.accent_soft
            } else {
                colors.text_high
            });
        let line = egui::RichText::new(look.line())
            .font(theme::small())
            .color(colors.text_low);
        let button = egui::Button::selectable(on, name)
            .right_text(line)
            .min_size(egui::vec2(ui.available_width(), theme::FIELD_H));
        if ui.add(button).clicked() {
            picked = Some(look);
        }
    }
    if current.is_none() {
        hint(ui, &tr!("layers-by-hand"));
    }
    picked
}

/// Why a setting here cannot be used on the printer in hand, and the way to the firmware
/// switch that would say otherwise. Answers whether that way was taken.
fn firmware_note(ui: &mut egui::Ui, why: &str) -> bool {
    hint(ui, why);
    ui.add_space(4.0);
    secondary_button(ui, icon::SETTINGS, &tr!("layers-open-printer")).clicked()
}

/// How many planes are sampled inside a layer. More finds a feature thinner than the
/// layer, and costs a slicing pass each.
fn samples_row(ui: &mut egui::Ui, samples: &mut NonZeroU8) {
    let mut value = f32::from(samples.get());
    if number_row(ui, &tr!("layers-planes"), &mut value, "", 1.0, 1.0..=5.0, 0) {
        *samples = NonZeroU8::new(value as u8).unwrap_or(ONE_SAMPLE);
    }
    if *samples != ONE_SAMPLE {
        describe(ui, &tr!("layers-planes-note"));
    }
}

/// How many greys an edge is rounded to. Off is every grey the panel will take.
fn grey_levels(ui: &mut egui::Ui, levels: &mut Option<NonZeroU8>) {
    let mut limited = levels.is_some();
    if switch(ui, &mut limited, &tr!("layers-round-grey")).changed() {
        *levels = limited.then_some(DEFAULT_GREY_LEVELS);
    }
    if let Some(count) = levels.as_mut() {
        nested(ui, |ui| {
            let mut value = f32::from(count.get());
            if number_row(
                ui,
                &tr!("layers-grey-levels"),
                &mut value,
                "",
                1.0,
                2.0..=16.0,
                0,
            ) {
                *count = NonZeroU8::new(value as u8).unwrap_or(DEFAULT_GREY_LEVELS);
            }
        });
    }
}

/// How many pixels either side an edge is faded over. Zero keeps it sharp.
fn blur_row(ui: &mut egui::Ui, blur_px: &mut u8) {
    let mut value = f32::from(*blur_px);
    if number_row(
        ui,
        &tr!("layers-edge-blur"),
        &mut value,
        &tr!("unit-px"),
        1.0,
        0.0..=8.0,
        0,
    ) {
        *blur_px = value as u8;
    }
    if *blur_px > 2 {
        describe(ui, &tr!("layers-blur-note"));
    }
}

/// Says out loud that changing the layer height carried the exposure along, and offers
/// it back; see `docs/decisions/0128`.
fn carried_note(ui: &mut egui::Ui, slicing: &mut crate::slicing::Slicing) {
    let Some(rescaled) = slicing.rescaled() else {
        return;
    };
    let text = tr!(
        "layers-carried",
        was = format!("{:.2}", rescaled.exposure_s),
        from = format!("{:.3}", rescaled.from_mm),
        now = format!("{:.2}", slicing.material.exposure_s)
    );
    let back = tr!(
        "field-back-to",
        value = format!("{:.2}", rescaled.exposure_s),
        unit = tr!("unit-s")
    );
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
        &tr!(
            "layers-thickest-reached",
            thickest = format!("{thickest:.3}"),
            asked = format!("{:.3}", material.layer_height_mm),
            exposure = format!("{exposure_s:.2}"),
            resin = format!("{:.2}", material.exposure_s)
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
            subheading_with_switch(ui, &tr!("layers-adaptive"), &mut on)
        })
        .inner;
    if flipped {
        slicing.adaptive = on.then(AdaptiveSettings::default);
    }
    let mut to_the_printer = false;
    if refused {
        to_the_printer = firmware_note(ui, &tr!("layers-adaptive-refused"));
    }
    let Some(adaptive) = slicing.adaptive.as_mut() else {
        return to_the_printer;
    };
    nested(ui, |ui| {
        number_row(
            ui,
            &tr!("layers-cusp"),
            &mut adaptive.cusp_mm,
            &tr!("unit-mm"),
            LAYER_HEIGHT_STEP,
            MIN_LAYER_HEIGHT_MM..=MAX_LAYER_HEIGHT_MM,
            3,
        );
        number_row(
            ui,
            &tr!("layers-thinnest"),
            &mut adaptive.min_height_mm,
            &tr!("unit-mm"),
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
        describe(ui, &tr!("layers-no-bands"));
    } else {
        describe(ui, &tr!("layers-bands-note"));
    }
    ui.add_space(4.0);

    if slicing.printer.is_some() && !slicing.printer_reads_per_layer() {
        return firmware_note(ui, &tr!("layers-bands-refused"));
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
        tone(
            ui,
            &tr!("layers-band", index = index + 1),
            theme::colors().text_mid,
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            remove = icon_button(ui, icon::REMOVE, &tr!("layers-remove-band")).clicked();
        });
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
    ui.add_space(4.0);
    remove
}

/// The square of colour a band is washed over the model in, drawn beside its fields so
/// the two can be told apart at a glance.
fn chip(ui: &mut egui::Ui, tint: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 2.0, tint);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_look_is_read_back_from_the_numbers_it_sets() {
        let mut slicing = crate::slicing::Slicing::default();
        for look in Look::ALL {
            look.apply(&mut slicing);
            let read = Look::of(slicing.anti_alias, slicing.grey_levels, slicing.blur_px);
            assert_eq!(read, Some(look), "{} did not survive", look.name());
        }
    }

    #[test]
    fn turning_smoothing_off_keeps_the_numbers_for_later() {
        let mut slicing = crate::slicing::Slicing::default();
        Look::Soft.apply(&mut slicing);
        Look::Off.apply(&mut slicing);
        assert!(!slicing.anti_alias);
        assert_eq!(
            slicing.blur_px, SOFT_BLUR_PX,
            "the soft blur waits to be turned on again"
        );
    }

    #[test]
    fn a_new_plate_starts_on_the_balanced_look() {
        let slicing = crate::slicing::Slicing::default();
        let look = Look::of(slicing.anti_alias, slicing.grey_levels, slicing.blur_px);
        assert_eq!(look, Some(Look::Balanced));
    }
}
