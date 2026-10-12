//! The corrections between what was sliced and what comes off the plate, and the two
//! calculators that work them out from a print. See `docs/design/compensation.md`.

use printer_profiles::{Compensation, layer_time_between, shrink_pct_between};

use crate::settings::{Calculator, Calculators};
use crate::ui::{
    card, describe, hint, icon, icon_button, inline_button, number_row, secondary_button, theme,
};

/// Millimetres, percent and seconds per point of drag on the fields here.
const PCT_STEP: f64 = 0.01;
const SECOND_STEP: f64 = 0.05;
const OFFSET_STEP: f64 = 0.001;

/// The widest wall move worth offering: past a tenth of a millimetre the model is wrong,
/// not the printer.
const OFFSET_RANGE: std::ops::RangeInclusive<f32> = -0.2..=0.2;

/// The widest correction worth offering: a resin off by a twentieth is a resin to replace.
const PCT_RANGE: std::ops::RangeInclusive<f32> = 95.0..=105.0;

/// Seconds a layer costs beyond what the settings account for. It goes negative: a print
/// that came in under its estimate is one the settings over-charge (BUG-47).
const LAYER_TIME_RANGE: std::ops::RangeInclusive<f32> = -30.0..=60.0;

/// The rows of the Compensation card: what the print comes out at, and the clock.
pub(super) fn rows(
    ui: &mut egui::Ui,
    values: &mut Compensation,
    calculators: &mut Calculators,
    pixel_mm: f32,
) {
    for (label, percent) in [
        ("Shrinkage X", &mut values.shrink_x_pct),
        ("Shrinkage Y", &mut values.shrink_y_pct),
    ] {
        number_row(ui, label, percent, "%", PCT_STEP, PCT_RANGE, 3);
    }
    number_row(
        ui,
        "Shrinkage Z",
        &mut values.shrink_z_pct,
        "%",
        PCT_STEP,
        PCT_RANGE,
        3,
    );
    describe(
        ui,
        "Above 100 slices the part larger, to measure right once the resin has shrunk. \
         Z is normally left alone: a layer's shrinkage along Z is taken up by the one above.",
    );
    if secondary_button(ui, "", "Work it out from a measured part").clicked() {
        calculators.open = Some(Calculator::Shrinkage);
    }

    ui.add_space(8.0);
    tolerance_rows(ui, values, pixel_mm);

    ui.add_space(8.0);
    number_row(
        ui,
        "Unaccounted per layer",
        &mut values.layer_time_s,
        "s",
        SECOND_STEP,
        LAYER_TIME_RANGE,
        2,
    );
    describe(
        ui,
        "Seconds the machine spends on a layer beyond exposure, waits and travel. \
         Negative where the estimate runs long. It only moves the estimate.",
    );
    if secondary_button(ui, "", "Work it out from a finished print").clicked() {
        calculators.open = Some(Calculator::LayerTime);
    }
}

/// How far each layer's walls move. `pixel_mm` is the panel's own pixel, which is the
/// finest edge it can draw and therefore what makes parity worth setting.
fn tolerance_rows(ui: &mut egui::Ui, values: &mut Compensation, pixel_mm: f32) {
    for (label, offset) in [
        ("Holes", &mut values.hole_offset_mm),
        ("Outer walls", &mut values.outer_offset_mm),
        ("Holes, bottom block", &mut values.bottom_hole_offset_mm),
        (
            "Outer walls, bottom block",
            &mut values.bottom_outer_offset_mm,
        ),
    ] {
        number_row(ui, label, offset, "mm", OFFSET_STEP, OFFSET_RANGE, 3);
    }
    describe(
        ui,
        "Positive leaves more material: a hole closes, an outer wall grows. Negative is \
         what takes off what the light bled past the mask, and on the bottom block what \
         takes off an elephant foot.",
    );

    number_row(
        ui,
        "Every second layer",
        &mut values.parity_offset_mm,
        "mm",
        OFFSET_STEP,
        OFFSET_RANGE,
        3,
    );
    if pixel_mm > 0.0 {
        describe(
            ui,
            &format!(
                "Added to both offsets on every second layer. This panel draws to \
                 {pixel_mm:.3} mm, so anything finer than that only exists as an average \
                 over alternating layers."
            ),
        );
    }
}

/// The calculator over the form, if one is open. Answers with what it worked out, once
/// the user takes it.
pub(super) fn calculator(
    ctx: &egui::Context,
    calculators: &mut Calculators,
    compensation: &mut Compensation,
) -> bool {
    let Some(which) = calculators.open else {
        return false;
    };
    let (mut taken, mut closed) = (false, false);
    let title = match which {
        Calculator::Shrinkage => "Shrinkage from a measured part",
        Calculator::LayerTime => "Unaccounted time from a print",
    };
    // A modal of its own, so it stands over the Machine and resin window.
    let modal = egui::Modal::new(egui::Id::new("compensation-calculator"))
        .frame(card().inner_margin(theme::PANEL_MARGIN))
        .show(ctx, |ui| {
            ui.set_width(380.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(title)
                        .font(theme::dialog_title())
                        .color(theme::colors().text_high),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    closed = icon_button(ui, icon::CANCEL, "Close  Esc").clicked();
                });
            });
            ui.add_space(6.0);
            taken = match which {
                Calculator::Shrinkage => shrinkage(ui, calculators, compensation),
                Calculator::LayerTime => layer_time(ui, calculators, compensation),
            };
        });
    if modal.should_close() || closed || taken {
        calculators.open = None;
    }
    taken
}

/// Drawn against measured, per axis. Answers whether the percentages were taken.
fn shrinkage(
    ui: &mut egui::Ui,
    calculators: &mut Calculators,
    compensation: &mut Compensation,
) -> bool {
    hint(
        ui,
        "Print a test part at 100 %, cure it, and measure it cold. Measure across the \
         plate for X and Y and up the build for Z.",
    );
    ui.add_space(6.0);
    for (axis, label) in ["X", "Y", "Z"].iter().enumerate() {
        number_row(
            ui,
            &format!("Drawn {label}"),
            &mut calculators.nominal_mm[axis],
            "mm",
            0.01,
            0.1..=400.0,
            3,
        );
        number_row(
            ui,
            &format!("Measured {label}"),
            &mut calculators.printed_mm[axis],
            "mm",
            0.01,
            0.1..=400.0,
            3,
        );
        ui.add_space(4.0);
    }

    let found = worked_out(calculators);
    match found {
        Some(percentages) => {
            hint(
                ui,
                &format!(
                    "{:.3} %, {:.3} % and {:.3} %.",
                    percentages[0], percentages[1], percentages[2]
                ),
            );
            for percent in percentages {
                if let Some(note) = limit_note(percent, &PCT_RANGE, "%") {
                    hint(ui, &note);
                    break;
                }
            }
        }
        None => hint(ui, "Every measurement has to be above zero."),
    }
    ui.add_space(6.0);
    let take = ui
        .horizontal(|ui| inline_button(ui, "", "Use these", true, found.is_some()).clicked())
        .inner;
    if let (true, Some([x, y, z])) = (take, found) {
        compensation.shrink_x_pct = held(x, &PCT_RANGE);
        compensation.shrink_y_pct = held(y, &PCT_RANGE);
        compensation.shrink_z_pct = held(z, &PCT_RANGE);
        return true;
    }
    false
}

/// `value` as the field will hold it. A correction past the field's range is held at its
/// limit, and `limit_note` is what says so rather than letting it happen quietly.
fn held(value: f32, range: &std::ops::RangeInclusive<f32>) -> f32 {
    value.clamp(*range.start(), *range.end())
}

/// What a worked-out value loses to the field it is going into, if it loses anything.
fn limit_note(value: f32, range: &std::ops::RangeInclusive<f32>, unit: &str) -> Option<String> {
    let kept = held(value, range);
    (kept != value).then(|| {
        format!(
            "The field takes {} to {} {unit}, so it will be held at {kept:.2}.",
            range.start(),
            range.end()
        )
    })
}

/// The three percentages, or `None` while a measurement is missing.
fn worked_out(calculators: &Calculators) -> Option<[f32; 3]> {
    let mut found = [0.0; 3];
    for (axis, percent) in found.iter_mut().enumerate() {
        *percent = shrink_pct_between(calculators.nominal_mm[axis], calculators.printed_mm[axis])?;
    }
    Some(found)
}

/// Predicted against the clock, over a known stack. Answers whether it was taken.
fn layer_time(
    ui: &mut egui::Ui,
    calculators: &mut Calculators,
    compensation: &mut Compensation,
) -> bool {
    hint(
        ui,
        "Both times have to come from the same print: what this window predicted for it, \
         and what the machine actually took.",
    );
    ui.add_space(6.0);
    clock_row(ui, "Predicted", &mut calculators.predicted);
    clock_row(ui, "Actual", &mut calculators.actual);
    number_row(
        ui,
        "Layers",
        &mut calculators.layers,
        "",
        1.0,
        0.0..=100_000.0,
        0,
    );

    let found = layer_time_between(
        Calculators::seconds(calculators.predicted),
        Calculators::seconds(calculators.actual),
        calculators.layers as u32,
    );
    match found {
        Some(seconds) => {
            hint(ui, &format!("{seconds:.2} s a layer."));
            if let Some(note) = limit_note(seconds, &LAYER_TIME_RANGE, "s") {
                hint(ui, &note);
            }
        }
        None => hint(ui, "A stack of no layers spreads nothing."),
    }
    ui.add_space(6.0);
    let take = ui
        .horizontal(|ui| inline_button(ui, "", "Use this", true, found.is_some()).clicked())
        .inner;
    if let (true, Some(seconds)) = (take, found) {
        // A print that beat its estimate gives a negative correction, and clamping it at
        // zero leaves the estimate wrong in the same direction for ever (BUG-47).
        compensation.layer_time_s = held(seconds, &LAYER_TIME_RANGE);
        return true;
    }
    false
}

/// Hours, minutes and seconds on one row.
fn clock_row(ui: &mut egui::Ui, label: &str, clock: &mut [f32; 3]) {
    number_row(
        ui,
        &format!("{label} hours"),
        &mut clock[0],
        "h",
        1.0,
        0.0..=99.0,
        0,
    );
    number_row(
        ui,
        &format!("{label} minutes"),
        &mut clock[1],
        "m",
        1.0,
        0.0..=59.0,
        0,
    );
    number_row(
        ui,
        &format!("{label} seconds"),
        &mut clock[2],
        "s",
        1.0,
        0.0..=59.0,
        0,
    );
    ui.add_space(4.0);
}
