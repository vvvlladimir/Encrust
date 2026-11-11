use std::borrow::Cow;

use egui::{Rect, Response, RichText, Sense, Ui, WidgetInfo, WidgetType, pos2, vec2};

use super::focus_ring;
use crate::ui::theme;

/// Track and knob of a switch, points.
const TRACK: egui::Vec2 = vec2(SWITCH_W, 16.0);
/// How wide a switch's track stands, points.
pub const SWITCH_W: f32 = 28.0;
const KNOB: f32 = 12.0;

/// A labelled switch filling one row: what it turns on, and the state it is in. A
/// disabled one still shows its state, dimmed, rather than disappearing.
pub fn switch(ui: &mut Ui, on: &mut bool, label: &str) -> Response {
    let size = vec2(ui.available_width(), theme::ROW_H);
    let (rect, response) = flip(ui, size, on, label);
    let track = Rect::from_min_size(
        pos2(rect.right() - TRACK.x, rect.center().y - TRACK.y / 2.0),
        TRACK,
    );
    paint_track(ui, &response, track, *on);
    let room = rect.with_max_x(track.left() - theme::ITEM_GAP);
    paint_label(ui, &response, room, label);
    response
}

/// How wide a brush's size box stands, points: room for `25.00 mm`.
const BRUSH_FIELD_W: f32 = 84.0;

/// What a `brush_row` was used for this frame.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct BrushUse {
    /// The brush was picked up or put down.
    pub toggled: bool,
    pub resized: bool,
}

/// A named brush on one row: its `icon_toggle`, lit while in hand, then its radius in a box,
/// millimetres, and at the far right the switch of what it paints for, where there is one.
pub fn brush_row(
    ui: &mut Ui,
    label: &str,
    (glyph, tooltip, brushing): (&str, &str, bool),
    radius_mm: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    switch: Option<&mut bool>,
) -> BrushUse {
    let size = vec2(ui.available_width(), theme::ROW_H);
    ui.allocate_ui_with_layout(
        size,
        egui::Layout::right_to_left(egui::Align::Center),
        |ui| {
            if let Some(on) = switch {
                let (track, response) = flip(ui, TRACK, on, label);
                paint_track(ui, &response, track, *on);
                ui.add_space(theme::ITEM_GAP);
            }
            let field = super::field::Field {
                name: label,
                unit: &crate::i18n::tr!("unit-mm"),
                width: BRUSH_FIELD_W,
                flagged: false,
            };
            let resized = super::field::boxed_field(ui, &field, radius_mm, 0.05, range, Some(2));
            ui.add_space(theme::ITEM_GAP);
            let toggled = super::icon_toggle(ui, glyph, tooltip, brushing).clicked();
            let room = ui
                .available_rect_before_wrap()
                .with_max_x(ui.cursor().right() - theme::ITEM_GAP);
            let hover = ui.interact(room, ui.id().with(label), Sense::hover());
            paint_label(ui, &hover, room, label);
            BrushUse { toggled, resized }
        },
    )
    .inner
}

/// A switch's label in `room`, cut with an ellipsis where a language runs it longer, and
/// then read whole on `response`'s tooltip.
fn paint_label(ui: &Ui, response: &Response, room: Rect, label: &str) {
    let colors = theme::colors();
    let ink = if ui.is_enabled() {
        colors.text_mid
    } else {
        colors.text_low
    };
    let galley = fitted(ui.painter(), label, room.width(), ink);
    let elided = galley.elided;
    let at = pos2(room.left(), room.center().y - galley.size().y / 2.0);
    ui.painter().galley(at, galley, ink);
    if elided {
        response.clone().on_hover_text(label);
    }
}

/// `label` on one line, cut with an ellipsis to `width_px`.
fn fitted(
    painter: &egui::Painter,
    label: &str,
    width_px: f32,
    ink: egui::Color32,
) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(label.to_owned(), theme::label(), ink);
    job.wrap = egui::text::TextWrapping::truncate_at_width(width_px.max(0.0));
    painter.layout_job(job)
}

/// The track alone, for a heading that already names what it turns on.
pub fn bare_switch(ui: &mut Ui, on: &mut bool, name: &str) -> Response {
    let (track, response) = flip(ui, TRACK, on, name);
    paint_track(ui, &response, track, *on);
    response
}

/// Takes the room a switch needs and turns `on` over when it is clicked.
fn flip(ui: &mut Ui, size: egui::Vec2, on: &mut bool, name: &str) -> (Rect, Response) {
    let enabled = ui.is_enabled();
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, enabled, *on, name));
    (rect, response)
}

fn paint_track(ui: &Ui, response: &Response, track: Rect, on: bool) {
    let colors = theme::colors();
    let enabled = ui.is_enabled();
    let how_on = theme::motion::toggle(ui.ctx(), response.id, on, theme::motion::FAST);
    let reached = if enabled { colors.accent } else { colors.line };
    let fill = colors.active.lerp_to_gamma(reached, how_on);
    let painter = ui.painter();
    painter.rect_filled(track, theme::R_PILL, fill);

    let travel = TRACK.x - KNOB - 4.0;
    let knob = Rect::from_min_size(
        pos2(
            track.left() + 2.0 + travel * how_on,
            track.center().y - KNOB / 2.0,
        ),
        egui::Vec2::splat(KNOB),
    );
    let knob_color = if enabled {
        colors.knob
    } else {
        colors.text_low
    };
    painter.rect_filled(knob, theme::R_PILL, knob_color);
    focus_ring(ui, response, theme::R_CONTROL);
}

/// Points kept clear either side of a segment's label, between the track and a pill, and
/// a pill's rounding.
const SEGMENT_PAD: f32 = 12.0;
const SEGMENT_INSET: f32 = 2.0;
const PILL_R: f32 = 4.0;

/// One choice inside a [`Segmented`] group.
pub struct Segment<'a, T> {
    pub value: T,
    pub label: Cow<'a, str>,
}

impl<'a, T> Segment<'a, T> {
    pub fn new(value: T, label: impl Into<Cow<'a, str>>) -> Self {
        Self {
            value,
            label: label.into(),
        }
    }
}

/// A row of mutually exclusive choices sunk in one bordered track, the chosen one a raised
/// pill ringed by a line, its words in the soft accent.
pub struct Segmented<'a, T> {
    segments: &'a [Segment<'a, T>],
    width: Option<f32>,
}

impl<'a, T: Copy + PartialEq> Segmented<'a, T> {
    pub fn new(segments: &'a [Segment<'a, T>]) -> Self {
        Self {
            segments,
            width: None,
        }
    }

    /// Spreads the segments evenly over `width` instead of sizing them to their labels.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Draws the group and writes the chosen value back. Returns whether it changed.
    pub fn show(self, ui: &mut Ui, current: &mut T) -> bool {
        let colors = theme::colors();
        let widths = self.widths(ui);
        let total: f32 = widths.iter().sum();

        // Segments hang off the group's own allocation: `ui.id()` is shared by every
        // widget in the parent, and a label by any two segments that read the same.
        let (rect, group) = ui.allocate_exact_size(vec2(total, theme::FIELD_H), Sense::hover());
        let under = ui.painter().add(egui::Shape::Noop);

        let mut changed = false;
        let mut left = rect.left();
        for (index, (segment, width)) in self.segments.iter().zip(widths).enumerate() {
            let slot = Rect::from_min_size(pos2(left, rect.top()), vec2(width, theme::FIELD_H));
            left += width;

            let response = ui.interact(slot, group.id.with(index), Sense::click());
            let active = *current == segment.value;
            if response.clicked() && !active {
                *current = segment.value;
                changed = true;
            }
            response.widget_info(|| {
                WidgetInfo::selected(
                    WidgetType::RadioButton,
                    ui.is_enabled(),
                    active,
                    &segment.label,
                )
            });
            Self::paint(ui, slot, segment, active, response.hovered());
            focus_ring(ui, &response, theme::R_CONTROL);
        }

        let frame = egui::epaint::RectShape::new(
            rect,
            theme::R_CONTROL,
            colors.sunken,
            egui::Stroke::new(1.0, colors.hairline),
            egui::StrokeKind::Inside,
        );
        ui.painter().set(under, frame);
        changed
    }

    fn paint(ui: &Ui, slot: Rect, segment: &Segment<'_, T>, active: bool, hovered: bool) {
        let colors = theme::colors();
        let (fill, ring, foreground) = match (active, hovered) {
            (true, _) => (colors.raised, colors.line, colors.accent_soft),
            (false, true) => (colors.hover, egui::Color32::TRANSPARENT, colors.text_high),
            (false, false) => (
                egui::Color32::TRANSPARENT,
                egui::Color32::TRANSPARENT,
                colors.text_mid,
            ),
        };

        let painter = ui.painter();
        let pill = slot.shrink(SEGMENT_INSET);
        let stroke = egui::Stroke::new(1.0, ring);
        painter.rect(pill, PILL_R, fill, stroke, egui::StrokeKind::Inside);

        let galley =
            painter.layout_no_wrap(segment.label.to_string(), theme::segment(), foreground);
        painter.galley(
            pos2(
                slot.center().x - galley.size().x / 2.0,
                slot.center().y - galley.size().y / 2.0,
            ),
            galley,
            foreground,
        );
    }

    /// Either an even share of the width the caller asked for, or what each label needs.
    fn widths(&self, ui: &Ui) -> Vec<f32> {
        if let Some(width) = self.width {
            let share = (width / self.segments.len() as f32).max(0.0);
            return vec![share; self.segments.len()];
        }

        self.segments
            .iter()
            .map(|segment| {
                let galley = ui.painter().layout_no_wrap(
                    segment.label.to_string(),
                    theme::segment(),
                    theme::colors().text_mid,
                );
                galley.size().x + 2.0 * SEGMENT_PAD
            })
            .collect()
    }
}

/// A label in a tone of the palette, for a note that is not a control.
pub fn tone(ui: &mut Ui, text: &str, color: egui::Color32) {
    ui.label(RichText::new(text).font(theme::small()).color(color));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_switch_label_longer_than_its_room_is_cut_and_a_short_one_kept_whole() {
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            theme::apply(ui.ctx());
            let ink = theme::colors().text_mid;
            let long = fitted(ui.painter(), "Чистая поверхность без опор", 60.0, ink);
            assert!(long.elided, "a label past 60 px is cut");
            assert!(
                long.size().x <= 60.0 + 0.5,
                "the cut label fits, got {}",
                long.size().x
            );
            let short = fitted(ui.painter(), "On", 60.0, ink);
            assert!(!short.elided, "a label inside its room is kept whole");
        });
        output.textures_delta.clear();
    }
}
