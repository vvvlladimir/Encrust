use std::ops::RangeInclusive;

use core_geometry::Scalar;
use egui::emath::Numeric;
use egui::{Color32, DragValue, RichText, TextStyle, Ui, vec2};

use super::icon_button;
use crate::ui::{icon, theme};

/// What a field is called, in `color`, with its unit in brackets after it: the one way
/// a number is labelled anywhere in the window.
pub fn field_label(ui: &mut Ui, text: &str, color: Color32, unit: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label(RichText::new(text).font(theme::label()).color(color));
        if !unit.is_empty() {
            ui.label(
                RichText::new(format!("({unit})"))
                    .font(theme::small())
                    .color(theme::colors().text_low),
            );
        }
    });
}

/// The letter of an axis in its colour, labelled like any other field.
pub fn axis_label(ui: &mut Ui, index: usize, unit: &str) {
    field_label(
        ui,
        ["X", "Y", "Z"][index],
        theme::colors().axis[index],
        unit,
    );
}

/// The one box a number is typed or dragged in, `width` points wide. `decimals` fixes
/// the digits after the point; `None` leaves them to the type. Returns whether it changed.
pub fn number_field<N: Numeric>(
    ui: &mut Ui,
    value: &mut N,
    speed: f64,
    range: RangeInclusive<N>,
    decimals: Option<usize>,
    width: f32,
) -> bool {
    ui.scope(|ui| {
        ui.style_mut()
            .text_styles
            .insert(TextStyle::Button, theme::mono(12.0));
        let mut field = DragValue::new(value).speed(speed).range(range);
        if let Some(decimals) = decimals {
            field = field.fixed_decimals(decimals);
        }
        ui.add_sized(vec2(width.max(40.0), theme::FIELD_H), field)
            .changed()
    })
    .inner
}

/// A named number: the label and its unit on the left, a box of one width on the right.
pub fn number_row(
    ui: &mut Ui,
    label: &str,
    value: &mut Scalar,
    unit: &str,
    speed: f64,
    range: RangeInclusive<Scalar>,
    decimals: usize,
) -> bool {
    row(ui, label, unit, |ui| {
        number_field(ui, value, speed, range, Some(decimals), theme::FIELD_W)
    })
}

/// What was done to a number the window had changed on the user's behalf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Carried {
    Untouched,
    Edited,
    /// The button that puts back what it was before the change.
    Reverted,
}

/// A named number the window changed on the user's behalf: while `was` is `Some`, its label
/// is in the warning colour and a button beside it puts that value back.
#[allow(clippy::too_many_arguments)]
pub fn carried_row(
    ui: &mut Ui,
    label: &str,
    value: &mut Scalar,
    unit: &str,
    speed: f64,
    range: RangeInclusive<Scalar>,
    decimals: usize,
    was: Option<Scalar>,
) -> Carried {
    let colors = theme::colors();
    let Some(was) = was else {
        return if number_row(ui, label, value, unit, speed, range, decimals) {
            Carried::Edited
        } else {
            Carried::Untouched
        };
    };
    let mut carried = Carried::Untouched;
    ui.horizontal(|ui| {
        field_label(ui, label, colors.warn, unit);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if number_field(ui, value, speed, range, Some(decimals), theme::FIELD_W) {
                carried = Carried::Edited;
            }
            let back = format!("Back to {was:.decimals$} {unit}");
            if icon_button(ui, icon::RESET, &back).clicked() {
                carried = Carried::Reverted;
            }
        });
    });
    carried
}

/// A named whole number, laid out like `number_row`: pixel counts and layer counts.
pub fn count_row(
    ui: &mut Ui,
    label: &str,
    value: &mut u32,
    unit: &str,
    speed: f64,
    range: RangeInclusive<u32>,
) -> bool {
    row(ui, label, unit, |ui| {
        number_field(ui, value, speed, range, None, theme::FIELD_W)
    })
}

fn row(ui: &mut Ui, label: &str, unit: &str, field: impl FnOnce(&mut Ui) -> bool) -> bool {
    ui.horizontal(|ui| {
        field_label(ui, label, theme::colors().text_mid, unit);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), field)
            .inner
    })
    .inner
}

/// A named line of text: the label on the left, the field taking what is left.
pub fn text_row(ui: &mut Ui, label: &str, value: &mut String) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        field_label(ui, label, theme::colors().text_mid, "");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let field = egui::TextEdit::singleline(value)
                .font(theme::label())
                .vertical_align(egui::Align::Center)
                .margin(egui::Margin::symmetric(8, 0))
                .background_color(theme::colors().raised);
            changed = ui
                .add_sized(vec2(ui.available_width(), theme::FIELD_H), field)
                .changed();
        });
    });
    changed
}
