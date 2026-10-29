//! The lists the top bar's chip opens: a machine or a resin to take in hand.

use egui::{Align2, Color32, Frame, Response, Sense, Ui, WidgetInfo, WidgetType, pos2, vec2};

use super::{focus_ring, two_lines};
use crate::ui::{icon, theme};

/// The frame a chooser hangs in under its part of the chip, on `raised` with a window's
/// shadow.
pub fn chooser_frame() -> Frame {
    let colors = theme::colors();
    Frame::new()
        .fill(colors.raised)
        .stroke(egui::Stroke::new(1.0, colors.hairline))
        .corner_radius(theme::R_SURFACE)
        .shadow(theme::WINDOW_SHADOW)
        .inner_margin(theme::TOOLBAR_PAD)
}

/// The name of a group of a chooser's rows, in capitals.
pub fn chooser_group(ui: &mut Ui, name: &str) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), GROUP_H), Sense::hover());
    ui.painter().text(
        rect.left_center() + vec2(ROW_PAD, 0.0),
        Align2::LEFT_CENTER,
        name.to_uppercase(),
        theme::rail_name(),
        theme::colors().text_low,
    );
}

/// What stands at the head of a chooser's row: a glyph, or a colour as a dot.
pub enum Lead<'a> {
    Glyph(&'a str),
    Dot(Color32),
}

/// What stands at a chooser row's right, before its tick: a dot, or a figure.
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
    let fill = match (row.chosen, response.hovered()) {
        (true, _) => colors.picked_wash,
        (false, true) => colors.hover,
        (false, false) => Color32::TRANSPARENT,
    };
    let painter = ui.painter();
    painter.rect_filled(rect, theme::R_CONTROL, fill);
    let lead_at = pos2(rect.left() + ROW_PAD + 8.0, rect.center().y);
    match row.lead {
        Lead::Glyph(glyph) => {
            painter.text(
                lead_at,
                Align2::CENTER_CENTER,
                glyph,
                theme::icon(16.0),
                colors.text_low,
            );
        }
        Lead::Dot(tint) => {
            painter.circle_filled(lead_at, DOT_R, tint);
        }
    }
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
            theme::icon(14.0),
            colors.accent_soft,
        );
    }
    let aside_right = tick_at.x - 14.0;
    match row.aside {
        Some(Aside::Dot(tint)) => {
            painter.circle_filled(pos2(aside_right - 4.0, rect.center().y), 3.5, tint);
        }
        Some(Aside::Text(text)) => {
            painter.text(
                pos2(aside_right, rect.center().y),
                Align2::RIGHT_CENTER,
                text,
                theme::figures(12.0),
                colors.text_mid,
            );
        }
        None => {}
    }
    focus_ring(ui, &response, theme::R_CONTROL);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A row under a chooser's rule that leads elsewhere: a glyph and what it does.
pub fn chooser_foot(ui: &mut Ui, glyph: &str, text: &str) -> Response {
    let colors = theme::colors();
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), FOOT_H), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, text));
    let (fill, ink) = if response.hovered() {
        (colors.hover, colors.text_high)
    } else {
        (Color32::TRANSPARENT, colors.text_mid)
    };
    let painter = ui.painter();
    painter.rect_filled(rect, theme::R_CONTROL, fill);
    let glyph_at = pos2(rect.left() + ROW_PAD + 8.0, rect.center().y);
    painter.text(
        glyph_at,
        Align2::CENTER_CENTER,
        glyph,
        theme::icon(14.0),
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
