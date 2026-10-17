//! The corrections between what was sliced and what comes off the plate, and the two
//! calculators that work them out from a print. See `docs/design/compensation.md`.

use printer_profiles::{Compensation, layer_time_between, shrink_pct_between};

use crate::i18n::tr;
use crate::settings::{Calculator, Calculators};
use egui::collapsing_header::CollapsingState;
use egui::vec2;

use crate::ui::{
    axis_label, card, hint, icon, icon_button, number_row, primary_button, secondary_button,
    subheading, subheading_with_switch, theme, unit_field,
};

mod diagram;

/// Millimetres, percent and seconds per point of drag on the fields here.
const PCT_STEP: f64 = 0.01;
const SECOND_STEP: f64 = 0.05;
const OFFSET_STEP: f64 = 0.001;

/// Wide enough never to stand in the way of a calibration; only nonsense is kept out.
const OFFSET_RANGE: std::ops::RangeInclusive<f32> = -10.0..=10.0;
const PCT_RANGE: std::ops::RangeInclusive<f32> = 50.0..=200.0;
const LAYER_TIME_RANGE: std::ops::RangeInclusive<f32> = -600.0..=600.0;

/// The rows of the Compensation card: each group under the switch that turns it on, and
/// the clock. See `docs/design/compensation.md`.
pub(super) fn rows(ui: &mut egui::Ui, values: &mut Compensation, calculators: &mut Calculators) {
    group(ui, &tr!("comp-shrinkage"), &mut values.shrinkage_on, |ui| {
        for (label, percent) in [
            ("X", &mut values.shrink_x_pct),
            ("Y", &mut values.shrink_y_pct),
            ("Z", &mut values.shrink_z_pct),
        ] {
            number_row(ui, label, percent, "%", PCT_STEP, PCT_RANGE, 3);
        }
        if secondary_button(ui, "", &tr!("comp-from-measured")).clicked() {
            calculators.open = Some(Calculator::Shrinkage);
        }
    });

    let (hole_mm, outer_mm) = (&mut values.hole_offset_mm, &mut values.outer_offset_mm);
    group(ui, &tr!("comp-tolerance"), &mut values.tolerance_on, |ui| {
        offsets(ui, hole_mm, outer_mm);
        number_row(
            ui,
            &tr!("comp-every-second-layer"),
            &mut values.parity_offset_mm,
            &tr!("unit-mm"),
            OFFSET_STEP,
            OFFSET_RANGE,
            3,
        );
    });

    let (hole_mm, outer_mm) = (
        &mut values.bottom_hole_offset_mm,
        &mut values.bottom_outer_offset_mm,
    );
    group(
        ui,
        &tr!("comp-bottom-tolerance"),
        &mut values.bottom_tolerance_on,
        |ui| {
            offsets(ui, hole_mm, outer_mm);
        },
    );

    subheading(ui, &tr!("comp-print-time"));
    number_row(
        ui,
        &tr!("comp-unaccounted"),
        &mut values.layer_time_s,
        &tr!("unit-s"),
        SECOND_STEP,
        LAYER_TIME_RANGE,
        2,
    );
    if secondary_button(ui, "", &tr!("comp-from-print")).clicked() {
        calculators.open = Some(Calculator::LayerTime);
    }
}

/// A group of fields under the switch that turns it on, folded away while it is off.
fn group(ui: &mut egui::Ui, title: &str, on: &mut bool, add: impl FnOnce(&mut egui::Ui)) {
    subheading_with_switch(ui, title, on);
    let id = ui.make_persistent_id(("compensation-group", title));
    let mut fold = CollapsingState::load_with_default_open(ui.ctx(), id, *on);
    fold.set_open(*on);
    fold.show_body_unindented(ui, add);
}

/// The drawing of `a` and `b` beside their two fields.
fn offsets(ui: &mut egui::Ui, hole_mm: &mut f32, outer_mm: &mut f32) {
    ui.horizontal_top(|ui| {
        diagram::tolerance(ui, *hole_mm, *outer_mm);
        ui.add_space(theme::BLOCK_GAP);
        ui.vertical(|ui| {
            ui.add_space(diagram::SIZE / 2.0 - theme::ROW_H);
            let mm = tr!("unit-mm");
            number_row(ui, "a", hole_mm, &mm, OFFSET_STEP, OFFSET_RANGE, 3);
            number_row(ui, "b", outer_mm, &mm, OFFSET_STEP, OFFSET_RANGE, 3);
        });
    });
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
        Calculator::Shrinkage => tr!("comp-shrinkage-title"),
        Calculator::LayerTime => tr!("comp-layer-time-title"),
    };
    // A modal of its own, so it stands over the Machine and resin window.
    let modal = egui::Modal::new(egui::Id::new("compensation-calculator"))
        .frame(card().inner_margin(theme::PANEL_MARGIN))
        .show(ctx, |ui| {
            ui.set_width(420.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(title)
                        .font(theme::dialog_title())
                        .color(theme::colors().text_high),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    closed = icon_button(ui, icon::CANCEL, &tr!("dialog-close")).clicked();
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
    hint(ui, &tr!("comp-shrinkage-note"));
    ui.add_space(10.0);
    egui::Grid::new("shrinkage-calculator")
        .num_columns(4)
        .spacing(vec2(10.0, 6.0))
        .show(ui, |ui| {
            ui.label("");
            for head in [tr!("comp-drawn"), tr!("comp-measured"), tr!("comp-scale")] {
                column_head(ui, &head);
            }
            ui.end_row();
            for (axis, label) in ["X", "Y", "Z"].into_iter().enumerate() {
                axis_label(ui, axis, "");
                for (name, size_mm) in [
                    (tr!("comp-drawn"), &mut calculators.nominal_mm[axis]),
                    (tr!("comp-measured"), &mut calculators.printed_mm[axis]),
                ] {
                    let name = format!("{name} {label}");
                    let mm = tr!("unit-mm");
                    unit_field(ui, &name, size_mm, &mm, 0.01, 0.1..=400.0, Some(3), SIZE_W);
                }
                let percent =
                    shrink_pct_between(calculators.nominal_mm[axis], calculators.printed_mm[axis]);
                figure(
                    ui,
                    &percent.map_or_else(|| "—".to_owned(), |p| format!("{p:.3} %")),
                );
                ui.end_row();
            }
        });

    let found = worked_out(calculators);
    let note = found.and_then(|found| {
        found
            .iter()
            .find_map(|percent| limit_note(*percent, &PCT_RANGE, "%"))
    });
    let take = take_button(ui, &tr!("comp-use-these"), found.is_some(), note.as_deref());
    if let (true, Some([x, y, z])) = (take, found) {
        compensation.shrink_x_pct = held(x, &PCT_RANGE);
        compensation.shrink_y_pct = held(y, &PCT_RANGE);
        compensation.shrink_z_pct = held(z, &PCT_RANGE);
        return true;
    }
    false
}

/// Width of a box in the calculators, points.
const SIZE_W: f32 = 112.0;
const CLOCK_W: f32 = 84.0;

fn column_head(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .font(theme::small())
            .color(theme::colors().text_low),
    );
}

fn figure(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .font(theme::figures(13.0))
            .color(theme::colors().text_high),
    );
}

/// The note on what the field will hold, if any, over the button that takes the result.
fn take_button(ui: &mut egui::Ui, text: &str, enabled: bool, note: Option<&str>) -> bool {
    ui.add_space(10.0);
    if let Some(note) = note {
        hint(ui, note);
        ui.add_space(4.0);
    }
    primary_button(ui, "", text, enabled).clicked()
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
        tr!(
            "comp-limit",
            least = range.start().to_string(),
            most = range.end().to_string(),
            unit = unit.to_owned(),
            kept = format!("{kept:.2}")
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
    hint(ui, &tr!("comp-same-print"));
    ui.add_space(10.0);
    egui::Grid::new("layer-time-calculator")
        .num_columns(4)
        .spacing(vec2(10.0, 6.0))
        .show(ui, |ui| {
            clock_row(ui, &tr!("comp-predicted"), &mut calculators.predicted);
            clock_row(ui, &tr!("comp-actual"), &mut calculators.actual);
            row_label(ui, &tr!("summary-layers"));
            unit_field(
                ui,
                &tr!("summary-layers"),
                &mut calculators.layers,
                "",
                1.0,
                0.0..=100_000.0,
                Some(0),
                CLOCK_W,
            );
            ui.end_row();
        });

    let found = layer_time_between(
        Calculators::seconds(calculators.predicted),
        Calculators::seconds(calculators.actual),
        calculators.layers as u32,
    );
    ui.add_space(10.0);
    egui::Frame::new()
        .fill(theme::colors().sunken)
        .corner_radius(theme::R_SURFACE)
        .inner_margin(theme::CARD_MARGIN)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                row_label(ui, &tr!("comp-per-layer"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    figure(
                        ui,
                        &found.map_or_else(
                            || "—".to_owned(),
                            |s| tr!("value-s", value = format!("{s:+.2}")),
                        ),
                    );
                });
            });
        });
    let note = found.and_then(|seconds| limit_note(seconds, &LAYER_TIME_RANGE, &tr!("unit-s")));
    let take = take_button(ui, &tr!("comp-use-this"), found.is_some(), note.as_deref());
    if let (true, Some(seconds)) = (take, found) {
        // A print that beat its estimate gives a negative correction, and clamping it at
        // zero leaves the estimate wrong in the same direction for ever (BUG-47).
        compensation.layer_time_s = held(seconds, &LAYER_TIME_RANGE);
        return true;
    }
    false
}

fn row_label(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .font(theme::label())
            .color(theme::colors().text_mid),
    );
}

/// Hours, minutes and seconds in three boxes on one grid row.
fn clock_row(ui: &mut egui::Ui, label: &str, clock: &mut [f32; 3]) {
    row_label(ui, label);
    for ((part, unit, most), value) in [
        (tr!("comp-hours"), tr!("unit-h"), 99.0),
        (tr!("comp-minutes"), tr!("unit-m"), 59.0),
        (tr!("comp-seconds"), tr!("unit-s"), 59.0),
    ]
    .into_iter()
    .zip(clock.iter_mut())
    {
        let name = format!("{label} {part}");
        unit_field(ui, &name, value, &unit, 1.0, 0.0..=most, Some(0), CLOCK_W);
    }
    ui.end_row();
}
