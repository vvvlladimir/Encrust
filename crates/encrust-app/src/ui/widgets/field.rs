use std::ops::RangeInclusive;

use core_geometry::Scalar;
use egui::emath::Numeric;
use egui::{
    Align, Color32, DragValue, Layout, Rect, RichText, Sense, TextStyle, Ui, UiBuilder, pos2, vec2,
};

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
    boxed_field(ui, value, speed, range, decimals, width, "")
}

/// Points kept clear either side of the unit inside a box.
const UNIT_PAD: f32 = 8.0;

/// The box itself: sunk below the panel, the figure at its right, and the unit, if any,
/// after it in the quietest tone. The drag and the typing are egui's own `DragValue`.
fn boxed_field<N: Numeric>(
    ui: &mut Ui,
    value: &mut N,
    speed: f64,
    range: RangeInclusive<N>,
    decimals: Option<usize>,
    width: f32,
    unit: &str,
) -> bool {
    let colors = theme::colors();
    let (rect, _) = ui.allocate_exact_size(vec2(width.max(40.0), theme::FIELD_H), Sense::hover());
    let frame = ui.painter().add(egui::Shape::Noop);
    let unit = (!unit.is_empty()).then(|| {
        ui.painter()
            .layout_no_wrap(unit.to_owned(), theme::unit(), colors.text_low)
    });
    let unit_w = unit.as_ref().map_or(0.0, |unit| unit.size().x + UNIT_PAD);
    let value_rect = Rect::from_min_max(rect.min, pos2(rect.right() - unit_w, rect.bottom()));

    let layout = Layout::top_down(Align::Max).with_main_align(Align::Center);
    let builder = UiBuilder::new().max_rect(value_rect).layout(layout);
    let response = ui
        .scope_builder(builder, |ui| {
            borderless(ui.style_mut(), value_rect.size());
            let mut field = DragValue::new(value).speed(speed).range(range);
            if let Some(decimals) = decimals {
                field = field.fixed_decimals(decimals);
            }
            ui.add(field)
        })
        .inner;

    let border = if response.has_focus() {
        colors.accent
    } else if ui.rect_contains_pointer(rect) && ui.is_enabled() {
        colors.line
    } else {
        colors.hairline
    };
    let shape = egui::epaint::RectShape::new(
        rect,
        theme::R_CONTROL,
        colors.sunken,
        egui::Stroke::new(1.0, border),
        egui::StrokeKind::Inside,
    );
    ui.painter().set(frame, shape);
    if let Some(unit) = unit {
        let at = pos2(rect.right() - unit_w + UNIT_PAD / 2.0, rect.center().y);
        ui.painter()
            .galley(at - vec2(0.0, unit.size().y / 2.0), unit, colors.text_low);
    }
    response.changed()
}

/// A `DragValue` that draws only its figure, filling `size`: the box around it is ours.
fn borderless(style: &mut egui::Style, size: egui::Vec2) {
    style
        .text_styles
        .insert(TextStyle::Button, theme::figures(12.0));
    style.spacing.interact_size = size;
    style.spacing.button_padding.x = UNIT_PAD;
    let visuals = &mut style.visuals;
    visuals.extreme_bg_color = Color32::TRANSPARENT;
    visuals.selection.stroke = egui::Stroke::NONE;
    for widget in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.bg_fill = Color32::TRANSPARENT;
        widget.weak_bg_fill = Color32::TRANSPARENT;
        widget.bg_stroke = egui::Stroke::NONE;
    }
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
    row(ui, label, |ui| {
        boxed_field(
            ui,
            value,
            speed,
            range,
            Some(decimals),
            theme::FIELD_W,
            unit,
        )
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
        field_label(ui, label, colors.warn, "");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let width = theme::FIELD_W;
            if boxed_field(ui, value, speed, range, Some(decimals), width, unit) {
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
    row(ui, label, |ui| {
        boxed_field(ui, value, speed, range, None, theme::FIELD_W, unit)
    })
}

fn row(ui: &mut Ui, label: &str, field: impl FnOnce(&mut Ui) -> bool) -> bool {
    ui.horizontal(|ui| {
        field_label(ui, label, theme::colors().text_mid, "");
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
                .background_color(theme::colors().sunken);
            changed = ui
                .add_sized(vec2(ui.available_width(), theme::FIELD_H), field)
                .changed();
        });
    });
    changed
}
