use egui::{Align2, Rect, Response, RichText, Sense, Ui, pos2, vec2};

use crate::ui::theme;

/// How long the knob takes to cross the track, seconds.
const SLIDE_S: f32 = 0.12;

/// Track and knob of a switch, points.
const TRACK: egui::Vec2 = vec2(32.0, 18.0);
const KNOB: f32 = 14.0;

/// A labelled switch filling one row: what it turns on, and the state it is in. A
/// disabled one still shows its state, dimmed, rather than disappearing.
pub fn switch(ui: &mut Ui, on: &mut bool, label: &str) -> Response {
    let colors = theme::colors();
    let enabled = ui.is_enabled();
    let (rect, mut response) =
        ui.allocate_exact_size(vec2(ui.available_width(), theme::ROW_H), Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }

    let how_on = ui.ctx().animate_bool_with_time(response.id, *on, SLIDE_S);

    let painter = ui.painter();
    painter.text(
        pos2(rect.left(), rect.center().y),
        Align2::LEFT_CENTER,
        label,
        theme::label(),
        if enabled {
            colors.text_mid
        } else {
            colors.text_low
        },
    );

    let track = Rect::from_min_size(
        pos2(rect.right() - TRACK.x, rect.center().y - TRACK.y / 2.0),
        TRACK,
    );
    let reached = if enabled { colors.accent } else { colors.line };
    let fill = colors.line.lerp_to_gamma(reached, how_on);
    painter.rect_filled(track, egui::CornerRadius::same(255), fill);

    let travel = TRACK.x - KNOB - 4.0;
    let knob = Rect::from_min_size(
        pos2(
            track.left() + 2.0 + travel * how_on,
            track.center().y - KNOB / 2.0,
        ),
        egui::Vec2::splat(KNOB),
    );
    let knob_color = match enabled {
        true => colors.text_high.lerp_to_gamma(colors.on_accent, how_on),
        false => colors.text_low,
    };
    painter.rect_filled(knob, egui::CornerRadius::same(255), knob_color);

    response
}

/// One choice inside a [`Segmented`] group.
pub struct Segment<'a, T> {
    pub value: T,
    pub label: &'a str,
}

impl<'a, T> Segment<'a, T> {
    pub fn new(value: T, label: &'a str) -> Self {
        Self { value, label }
    }
}

/// A pill of mutually exclusive choices, such as the mode switch.
///
/// The filled flavour marks the chosen segment with the accent itself and is for the one
/// switch that changes what the whole window is doing. Everything else uses the wash.
pub struct Segmented<'a, T> {
    segments: &'a [Segment<'a, T>],
    filled: bool,
    width: Option<f32>,
    height: f32,
}

impl<'a, T: Copy + PartialEq> Segmented<'a, T> {
    pub fn new(segments: &'a [Segment<'a, T>]) -> Self {
        Self {
            segments,
            filled: false,
            width: None,
            height: theme::ROW_H,
        }
    }

    pub fn filled(mut self) -> Self {
        self.filled = true;
        self
    }

    /// Spreads the segments evenly over `width` instead of sizing them to their labels.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Makes the group `height` points tall instead of a row's height.
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// Draws the group and writes the chosen value back. Returns whether it changed.
    pub fn show(self, ui: &mut Ui, current: &mut T) -> bool {
        let colors = theme::colors();
        let padding = 3.0;
        let widths = self.widths(ui);
        let height = self.height;
        let total: f32 = widths.iter().sum::<f32>() + padding * 2.0;

        // Segments hang off the group's own allocation: `ui.id()` is shared by every
        // widget in the parent, and a label by any two segments that read the same.
        let (rect, group) = ui.allocate_exact_size(vec2(total, height), Sense::hover());
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(255), colors.raised);

        let mut changed = false;
        let mut left = rect.left() + padding;
        for (index, (segment, width)) in self.segments.iter().zip(widths).enumerate() {
            let slot = Rect::from_min_size(
                pos2(left, rect.top() + padding),
                vec2(width, height - padding * 2.0),
            );
            left += width;

            let response = ui.interact(slot, group.id.with(index), Sense::click());
            let active = *current == segment.value;
            if response.clicked() && !active {
                *current = segment.value;
                changed = true;
            }
            self.paint(ui, slot, segment, active, response.hovered());
        }
        changed
    }

    fn paint(&self, ui: &Ui, slot: Rect, segment: &Segment<'_, T>, active: bool, hovered: bool) {
        let colors = theme::colors();
        let (fill, foreground) = match (active, self.filled, hovered) {
            (true, true, _) => (colors.accent, colors.on_accent),
            (true, false, _) => (colors.accent_wash, colors.accent),
            (false, _, true) => (egui::Color32::TRANSPARENT, colors.text_high),
            (false, _, false) => (egui::Color32::TRANSPARENT, colors.text_mid),
        };

        let painter = ui.painter();
        painter.rect_filled(slot, egui::CornerRadius::same(255), fill);

        let font = theme::label();
        let galley = painter.layout_no_wrap(segment.label.to_owned(), font, foreground);
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
            let share = ((width - 6.0) / self.segments.len() as f32).max(0.0);
            return vec![share; self.segments.len()];
        }

        self.segments
            .iter()
            .map(|segment| {
                let galley = ui.painter().layout_no_wrap(
                    segment.label.to_owned(),
                    theme::label(),
                    theme::colors().text_mid,
                );
                galley.size().x + 32.0
            })
            .collect()
    }
}

/// A label in a tone of the palette, for a note that is not a control.
pub fn tone(ui: &mut Ui, text: &str, color: egui::Color32) {
    ui.label(RichText::new(text).font(theme::small()).color(color));
}
