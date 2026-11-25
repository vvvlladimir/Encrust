//! The lists the top bar's chip opens: a machine or a resin to take in hand.

use egui::{
    Align, Align2, Color32, Frame, Layout, Response, Sense, Ui, UiBuilder, WidgetInfo, WidgetType,
    pos2, vec2,
};

use super::{focus_ring, hover_mix, two_lines, two_lines_in};
use crate::ui::{icon, theme};

/// The frame a chooser hangs in under its part of the chip, on `raised` with a window's
/// shadow.
pub fn chooser_frame() -> Frame {
    let colors = theme::colors();
    Frame::new()
        .fill(colors.raised)
        .stroke(egui::Stroke::new(1.0, colors.hairline))
        .corner_radius(theme::R_SURFACE)
        .shadow(theme::window_shadow())
        .inner_margin(theme::TOOLBAR_PAD)
}

/// The name of a group of a chooser's rows, in capitals.
pub fn chooser_group(ui: &mut Ui, name: &str) {
    capitals(ui, name, ROW_PAD);
}

/// The name of a group of rows standing flush with them, in capitals: a window's list, the
/// sheet of keys.
pub fn group_name(ui: &mut Ui, name: &str) {
    capitals(ui, name, 0.0);
}

fn capitals(ui: &mut Ui, name: &str, pad: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), GROUP_H), Sense::hover());
    ui.painter().text(
        rect.left_center() + vec2(pad, 0.0),
        Align2::LEFT_CENTER,
        name.to_uppercase(),
        theme::rail_name(),
        theme::colors().text_low,
    );
}

/// What stands at the head of a chooser's row: a glyph, or a colour as a dot.
#[derive(Clone, Copy)]
pub enum Lead<'a> {
    Glyph(&'a str),
    Dot(Color32),
}

impl Lead<'_> {
    /// Paints the lead centred on `at`, a glyph of `glyph_px` or a dot of `dot_r`.
    pub(super) fn paint(self, painter: &egui::Painter, at: egui::Pos2, glyph_px: f32, dot_r: f32) {
        match self {
            Self::Glyph(glyph) => {
                let low = theme::colors().text_low;
                painter.text(at, Align2::CENTER_CENTER, glyph, theme::icon(glyph_px), low);
            }
            Self::Dot(tint) => {
                painter.circle_filled(at, dot_r, tint);
            }
        }
    }
}

/// What stands at a chooser row's right, before its tick: a dot, or a figure.
#[derive(Clone, Copy)]
pub enum Aside<'a> {
    Dot(Color32),
    Text(&'a str),
}

/// One choice of a chooser: its lead, its name over what says more about it, an aside, and
/// while it is the one in hand a wash and a tick.
pub struct ChooserRow<'a> {
    pub lead: Lead<'a>,
    pub name: &'a str,
    pub detail: &'a str,
    pub aside: Option<Aside<'a>>,
    pub chosen: bool,
}

/// Draws a [`ChooserRow`], 44 tall.
pub fn chooser_row(ui: &mut Ui, row: &ChooserRow<'_>) -> Response {
    let colors = theme::colors();
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    response.widget_info(|| {
        WidgetInfo::selected(WidgetType::SelectableLabel, true, row.chosen, row.name)
    });
    let fill = match row.chosen {
        true => colors.picked_wash,
        false => hover_mix(ui, &response, Color32::TRANSPARENT, colors.hover),
    };
    let painter = ui.painter();
    painter.rect_filled(rect, theme::R_CONTROL, fill);
    let lead_at = pos2(rect.left() + ROW_PAD + 8.0, rect.center().y);
    row.lead.paint(painter, lead_at, 16.0, DOT_R);
    two_lines(
        painter,
        pos2(lead_at.x + 16.0, rect.center().y),
        row.name,
        row.detail,
    );
    let tick_at = pos2(rect.right() - ROW_PAD - 7.0, rect.center().y);
    if row.chosen {
        painter.text(
            tick_at,
            Align2::CENTER_CENTER,
            icon::IN_HAND,
            theme::icon(theme::glyph::ROW),
            colors.accent_soft,
        );
    }
    paint_aside(painter, pos2(tick_at.x - 14.0, rect.center().y), row.aside);
    focus_ring(ui, &response, theme::R_CONTROL);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A [`ChooserRow`] whose right end holds what `end` lays out right to left, in place of
/// its tick: the row's own actions. Its lines are cut before them, whole on hover.
pub fn chooser_row_with(ui: &mut Ui, row: &ChooserRow<'_>, end: impl FnOnce(&mut Ui)) -> Response {
    let colors = theme::colors();
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, row.name));
    let fill = hover_mix(ui, &response, Color32::TRANSPARENT, colors.hover);
    ui.painter().rect_filled(rect, theme::R_CONTROL, fill);
    // A child laid out after the row's own click, so its buttons win the press over it.
    let inner = rect.shrink2(vec2(END_PAD, 0.0));
    let layout = Layout::right_to_left(Align::Center);
    let mut actions = ui.new_child(UiBuilder::new().max_rect(inner).layout(layout));
    end(&mut actions);
    let actions_left = actions.min_rect().left().min(inner.right());
    let painter = ui.painter();
    let lead_at = pos2(rect.left() + ROW_PAD + 8.0, rect.center().y);
    row.lead.paint(painter, lead_at, 16.0, DOT_R);
    let aside_right = actions_left - theme::ITEM_GAP;
    paint_aside(painter, pos2(aside_right, rect.center().y), row.aside);
    let text_left = lead_at.x + 16.0;
    let room = aside_right - ASIDE_ROOM - text_left;
    let cut = two_lines_in(
        painter,
        pos2(text_left, rect.center().y),
        (row.name, row.detail),
        room,
    );
    focus_ring(ui, &response, theme::R_CONTROL);
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    match cut {
        true => response.on_hover_text(format!("{}\n{}", row.name, row.detail)),
        false => response,
    }
}

/// A switch under a chooser's rule, its label in line with the rows' names above it.
pub fn chooser_switch(ui: &mut Ui, on: &mut bool, label: &str) -> Response {
    let (rect, _) =
        ui.allocate_exact_size(vec2(ui.available_width(), theme::ROW_H), Sense::hover());
    let inner = rect.shrink2(vec2(ROW_PAD, 0.0));
    let mut child = ui.new_child(UiBuilder::new().max_rect(inner));
    super::switch(&mut child, on, label)
}

/// What stands left of the tick, its right edge at `right_at`.
fn paint_aside(painter: &egui::Painter, right_at: egui::Pos2, aside: Option<Aside<'_>>) {
    match aside {
        Some(Aside::Dot(tint)) => {
            painter.circle_filled(right_at - vec2(4.0, 0.0), 3.5, tint);
        }
        Some(Aside::Text(text)) => {
            let mid = theme::colors().text_mid;
            painter.text(
                right_at,
                Align2::RIGHT_CENTER,
                text,
                theme::figures(12.0),
                mid,
            );
        }
        None => {}
    }
}

/// A row under a chooser's rule that leads elsewhere: a glyph and what it does.
pub fn chooser_foot(ui: &mut Ui, glyph: &str, text: &str) -> Response {
    let colors = theme::colors();
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), FOOT_H), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, text));
    let fill = hover_mix(ui, &response, Color32::TRANSPARENT, colors.hover);
    let ink = if response.hovered() {
        colors.text_high
    } else {
        colors.text_mid
    };
    let painter = ui.painter();
    painter.rect_filled(rect, theme::R_CONTROL, fill);
    let glyph_at = pos2(rect.left() + ROW_PAD + 8.0, rect.center().y);
    painter.text(
        glyph_at,
        Align2::CENTER_CENTER,
        glyph,
        theme::icon(theme::glyph::ROW),
        ink,
    );
    painter.text(
        pos2(glyph_at.x + 16.0, rect.center().y),
        Align2::LEFT_CENTER,
        text,
        theme::label(),
        ink,
    );
    focus_ring(ui, &response, theme::R_CONTROL);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A group's name, a choice and a foot row, how far in each starts, and a colour's dot.
const GROUP_H: f32 = 24.0;
const ROW_H: f32 = 44.0;
const FOOT_H: f32 = 32.0;
const ROW_PAD: f32 = 8.0;
const DOT_R: f32 = 5.0;
/// How far a row's actions stand in from its right, and the room kept for its aside.
const END_PAD: f32 = 4.0;
const ASIDE_ROOM: f32 = 12.0;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::widgets::fixtures;

    #[test]
    fn a_chooser_row_stands_two_lines_tall_whatever_it_holds() {
        let ctx = fixtures::context();
        let row = ChooserRow {
            lead: Lead::Glyph(icon::PRINTER),
            name: "Saturn 4 Ultra",
            detail: "Elegoo · .goo",
            aside: Some(Aside::Dot(Color32::GREEN)),
            chosen: true,
        };
        let mut height = None;
        fixtures::frame(&ctx, egui::RawInput::default(), |ui| {
            let response = chooser_row(ui, &row);
            height = Some(response.rect.height());
        });
        assert_eq!(height, Some(ROW_H));
    }
}
