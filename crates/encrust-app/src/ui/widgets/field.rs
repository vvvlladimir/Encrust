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

/// The one box a number is typed or dragged in, `width` points wide, called `name` to a
/// screen reader. `decimals` fixes the digits after the point; `None` leaves them to the
/// type. Returns whether it changed.
pub fn number_field<N: Numeric>(
    ui: &mut Ui,
    name: &str,
    value: &mut N,
    speed: f64,
    range: RangeInclusive<N>,
    decimals: Option<usize>,
    width: f32,
) -> bool {
    let field = Field {
        name,
        unit: "",
        width,
    };
    boxed_field(ui, &field, value, speed, range, decimals)
}

/// A `number_field` with its unit drawn inside the box, for a row of several boxes that
/// share one label.
#[allow(clippy::too_many_arguments)]
pub fn unit_field<N: Numeric>(
    ui: &mut Ui,
    name: &str,
    value: &mut N,
    unit: &str,
    speed: f64,
    range: RangeInclusive<N>,
    decimals: Option<usize>,
    width: f32,
) -> bool {
    let field = Field { name, unit, width };
    boxed_field(ui, &field, value, speed, range, decimals)
}

/// What a box is called, the unit drawn inside it, and its width in points.
struct Field<'a> {
    name: &'a str,
    unit: &'a str,
    width: f32,
}

/// Names a control for a screen reader, which cannot see the label painted beside it.
fn name_for_reader(ui: &Ui, id: egui::Id, name: &str) {
    ui.ctx()
        .accesskit_node_builder(id, |node| node.set_label(name));
}

/// Points kept clear either side of the unit inside a box.
const UNIT_PAD: f32 = 8.0;

/// The box itself: sunk below the panel, the figure at its right, and the unit, if any,
/// after it in the quietest tone. The drag and the typing are egui's own `DragValue`.
fn boxed_field<N: Numeric>(
    ui: &mut Ui,
    field: &Field<'_>,
    value: &mut N,
    speed: f64,
    range: RangeInclusive<N>,
    decimals: Option<usize>,
) -> bool {
    let colors = theme::colors();
    let unit = field.unit;
    let size = vec2(field.width.max(40.0), theme::FIELD_H);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
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
            // egui would otherwise pull a value outside `range` into it every frame, and
            // report that as an edit: a model sunk under the plate would be lifted back.
            // A typed figure is taken when the box is left, not at each key: the first
            // digit of 10 is under a least of 2 and would be pulled up to it at once.
            let mut drag = DragValue::new(value)
                .speed(speed)
                .range(range)
                .clamp_existing_to_range(false)
                .update_while_editing(false);
            if let Some(decimals) = decimals {
                drag = drag.fixed_decimals(decimals);
            }
            ui.add(drag)
        })
        .inner;
    name_for_reader(ui, response.id, field.name);

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
    // egui paints selected text in this stroke's colour, so only its width may go.
    visuals.selection.stroke.width = 0.0;
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
    let field = Field {
        name: label,
        unit,
        width: theme::FIELD_W,
    };
    row(ui, label, |ui| {
        boxed_field(ui, &field, value, speed, range, Some(decimals))
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
            let field = Field {
                name: label,
                unit,
                width: theme::FIELD_W,
            };
            if boxed_field(ui, &field, value, speed, range, Some(decimals)) {
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
    let field = Field {
        name: label,
        unit,
        width: theme::FIELD_W,
    };
    row(ui, label, |ui| {
        boxed_field(ui, &field, value, speed, range, None)
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

/// The box an IP address is typed into, as tall as the buttons beside it and `width`
/// points wide.
pub fn address_field(ui: &mut Ui, text: &mut String, hint: &str, width: f32) -> egui::Response {
    let field = text_box(egui::TextEdit::singleline(text))
        .hint_text(hint)
        .font(theme::figures(13.0))
        .background_color(theme::colors().sunken);
    let response = ui.add_sized(vec2(width, theme::FIELD_H), field);
    name_for_reader(ui, response.id, "Printer address");
    response
}

/// A line of text in the one shape every typed box has: as tall as a button, its text
/// centred in it.
pub fn text_box(edit: egui::TextEdit<'_>) -> egui::TextEdit<'_> {
    edit.min_size(vec2(0.0, theme::FIELD_H))
        .vertical_align(Align::Center)
        .margin(egui::Margin::symmetric(10, 0))
}

/// A named line of text: the label on the left, the field taking what is left.
pub fn text_row(ui: &mut Ui, label: &str, value: &mut String) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        field_label(ui, label, theme::colors().text_mid, "");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let field = text_box(egui::TextEdit::singleline(value))
                .font(theme::label())
                .background_color(theme::colors().sunken);
            let response = ui.add_sized(vec2(ui.available_width(), theme::FIELD_H), field);
            name_for_reader(ui, response.id, label);
            changed = response.changed();
        });
    });
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_value_outside_the_range_is_left_alone_until_it_is_edited() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let mut bottom_mm = -5.0_f32;
        let mut changed = false;
        for _ in 0..2 {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                changed |=
                    number_field(ui, "Lift", &mut bottom_mm, 0.1, 0.0..=100.0, Some(2), 80.0);
            });
            // No renderer takes the font atlas here.
            output.textures_delta.clear();
        }
        assert_eq!(bottom_mm, -5.0, "a model sunk under the plate stays there");
        assert!(!changed, "nobody touched the field");
    }

    #[test]
    fn a_figure_is_taken_when_the_box_is_left_and_not_at_each_key() {
        use egui_kittest::kittest::Queryable as _;

        let mut dressed = false;
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(320.0, 80.0))
            .build_ui_state(
                move |ui, diameter_mm: &mut f32| {
                    if dressed {
                        number_field(ui, "Diameter", diameter_mm, 0.05, 2.0..=50.0, Some(2), 80.0);
                    } else {
                        theme::apply(ui.ctx());
                        dressed = true;
                    }
                },
                20.7_f32,
            );
        harness.run_steps(3);
        harness.get_by_label("Diameter").click();
        harness.run_steps(3);

        harness.get_by_label("Diameter").type_text("1");
        harness.run_steps(3);
        assert_eq!(
            *harness.state(),
            20.7,
            "1 is under the least of 2, and is only the first digit of 10"
        );

        harness.get_by_label("Diameter").type_text("0");
        harness.run_steps(3);
        harness.key_press(egui::Key::Enter);
        harness.run_steps(3);
        assert_eq!(*harness.state(), 10.0, "what was typed is what is taken");
    }
}
