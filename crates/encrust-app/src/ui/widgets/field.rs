use std::ops::RangeInclusive;

use egui::emath::Numeric;
use egui::{Align, Color32, DragValue, Layout, Rect, Sense, TextStyle, Ui, UiBuilder, pos2, vec2};

use crate::ui::theme;

mod rows;
mod text;

pub(in crate::ui::widgets) use rows::row;
pub use rows::{Carried, carried_row, count_row, number_row, picker_row};
pub use text::{address_field, row_name, text_box, text_row};

/// What a field is called, in `color`, with its unit in brackets after it: the one way
/// a number is labelled anywhere in the window. Cut short with an ellipsis, never wider
/// than the room it is given.
pub fn field_label(ui: &mut Ui, text: &str, color: Color32, unit: &str) {
    let mut job = egui::text::LayoutJob::default();
    let format = |font_id, color| egui::TextFormat {
        font_id,
        color,
        valign: Align::Center,
        ..Default::default()
    };
    job.append(text, 0.0, format(theme::label(), color));
    if !unit.is_empty() {
        let unit = format!("({unit})");
        job.append(&unit, 4.0, format(theme::small(), theme::colors().text_low));
    }
    ui.add(egui::Label::new(job).truncate());
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
        flagged: false,
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
    let field = Field {
        name,
        unit,
        width,
        flagged: false,
    };
    boxed_field(ui, &field, value, speed, range, decimals)
}

/// What a box is called, the unit drawn inside it, its width in points, and whether its
/// value strays from what it should be, which borders it in `danger`.
pub(in crate::ui::widgets) struct Field<'a> {
    pub(super) name: &'a str,
    pub(super) unit: &'a str,
    pub(super) width: f32,
    pub(super) flagged: bool,
}

/// Names a control for a screen reader, which cannot see the label painted beside it.
pub(super) fn name_for_reader(ui: &Ui, id: egui::Id, name: &str) {
    ui.ctx()
        .accesskit_node_builder(id, |node| node.set_label(name));
}

/// Points kept clear either side of the unit inside a box.
const UNIT_PAD: f32 = 8.0;

/// The units whose slot every box shares, so the figures of a column stand on one line
/// whichever unit follows them; a longer unit widens its own box's slot.
const SLOT_UNITS: [&str; 7] = [
    "unit-mm",
    "unit-mm2",
    "unit-mm3",
    "unit-um",
    "unit-deg",
    "unit-times",
    "unit-px",
];

/// How wide the unit's slot is in a box: the widest of `SLOT_UNITS`, or `unit` if wider.
fn unit_slot_w(ui: &Ui, unit: &egui::Galley) -> f32 {
    let low = theme::colors().text_low;
    let widest = SLOT_UNITS
        .iter()
        .map(|key| {
            let text = crate::i18n::text(key, None);
            ui.painter()
                .layout_no_wrap(text, theme::unit(), low)
                .size()
                .x
        })
        .fold(0.0, f32::max);
    widest.max(unit.size().x)
}

/// The decimals the standard gives a quantity in `unit` (`docs/design/ui-standards.md`,
/// Numbers), so a field cannot show its own; `None` for a unit the table lacks.
fn standard_decimals(unit: &str) -> Option<usize> {
    [
        ("unit-mm", 2),
        ("unit-um", 0),
        ("unit-deg", 1),
        ("unit-s", 2),
        ("unit-mm-min", 1),
        ("unit-mm3", 1),
        ("unit-times", 2),
        ("unit-px", 0),
    ]
    .into_iter()
    .find_map(|(key, decimals)| (crate::i18n::text(key, None) == unit).then_some(decimals))
}

/// The box itself: sunk below the panel, the figure at its right, and the unit, if any,
/// after it in the quietest tone. The drag and the typing are egui's own `DragValue`.
pub(in crate::ui::widgets) fn boxed_field<N: Numeric>(
    ui: &mut Ui,
    field: &Field<'_>,
    value: &mut N,
    speed: f64,
    range: RangeInclusive<N>,
    decimals: Option<usize>,
) -> bool {
    let colors = theme::colors();
    let unit = field.unit;
    let decimals = standard_decimals(unit).or(decimals);
    let size = vec2(field.width.max(40.0), theme::FIELD_H);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let frame = ui.painter().add(egui::Shape::Noop);
    let unit = (!unit.is_empty()).then(|| {
        ui.painter()
            .layout_no_wrap(unit.to_owned(), theme::unit(), colors.text_low)
    });
    let unit_w = unit
        .as_ref()
        .map_or(0.0, |unit| unit_slot_w(ui, unit) + UNIT_PAD);
    let value_rect = Rect::from_min_max(rect.min, pos2(rect.right() - unit_w, rect.bottom()));

    let response = drag_figure(ui, value_rect, value, (speed, range), decimals);
    // The scope above leaves the cursor after the figure, short of the unit: put it back
    // after the whole box, or the next control in a row would sit over the unit.
    ui.advance_cursor_after_rect(rect);
    name_for_reader(ui, response.id, field.name);
    paint_box(ui, frame, rect, &response, field.flagged);
    if let Some(unit) = unit {
        let at = pos2(rect.right() - unit_w + UNIT_PAD / 2.0, rect.center().y);
        ui.painter()
            .galley(at - vec2(0.0, unit.size().y / 2.0), unit, colors.text_low);
    }
    response.changed()
}

/// The figure of a box, dragged or typed in `value_rect` at `speed` within `range`.
fn drag_figure<N: Numeric>(
    ui: &mut Ui,
    value_rect: Rect,
    value: &mut N,
    (speed, range): (f64, RangeInclusive<N>),
    decimals: Option<usize>,
) -> egui::Response {
    let layout = Layout::top_down(Align::Max).with_main_align(Align::Center);
    let builder = UiBuilder::new().max_rect(value_rect).layout(layout);
    ui.scope_builder(builder, |ui| {
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
    .inner
}

/// The sunken box behind a figure, into the `frame` kept for it under the figure: ringed in
/// the accent while typed in, the danger while `flagged`, `line` under the pointer.
fn paint_box(
    ui: &Ui,
    frame: egui::layers::ShapeIdx,
    rect: Rect,
    response: &egui::Response,
    flagged: bool,
) {
    let colors = theme::colors();
    let border = if response.has_focus() {
        colors.accent
    } else if flagged {
        colors.danger
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
}

/// A `DragValue` that draws only its figure, filling `size`: the box around it is ours.
fn borderless(style: &mut egui::Style, size: egui::Vec2) {
    style
        .text_styles
        .insert(TextStyle::Button, theme::figures(13.0));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_unit_of_the_standard_brings_its_own_decimals() {
        assert_eq!(standard_decimals("mm"), Some(2));
        assert_eq!(standard_decimals("°"), Some(1));
        assert_eq!(standard_decimals("µm"), Some(0));
        assert_eq!(
            standard_decimals("g/ml"),
            None,
            "a unit the table lacks keeps the caller's"
        );
    }

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
