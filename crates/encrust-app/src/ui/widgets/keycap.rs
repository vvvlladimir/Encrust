use egui::{Painter, Rect, Stroke, StrokeKind, pos2, vec2};

use crate::ui::theme;

/// Room either side of a key's face, and the cap's height; its lower edge is drawn thicker.
const CAP_PAD_X: f32 = 5.0;
const CAP_H: f32 = 18.0;
const CAP_MIN_W: f32 = 18.0;

/// The width a keycap saying `text` takes.
fn keycap_width(painter: &Painter, text: &str) -> f32 {
    let galley = painter.layout_no_wrap(text.to_owned(), theme::keycap(), theme::colors().text_mid);
    (galley.size().x + CAP_PAD_X * 2.0).max(CAP_MIN_W)
}

/// One key as printed on the keyboard, drawn leftwards from `right`; returns its left edge,
/// so the next cap of a chord knows where to go.
pub fn keycap(painter: &Painter, text: &str, right: f32, center_y: f32) -> f32 {
    let colors = theme::colors();
    let width = keycap_width(painter, text);
    let cap = Rect::from_min_size(
        pos2(right - width, center_y - CAP_H / 2.0),
        vec2(width, CAP_H),
    );
    let radius = egui::CornerRadius::same(4);
    painter.rect(cap, radius, colors.line, Stroke::NONE, StrokeKind::Inside);
    let face = Rect::from_min_max(cap.min + vec2(1.0, 1.0), cap.max - vec2(1.0, 2.0));
    painter.rect(
        face,
        radius,
        colors.raised,
        Stroke::NONE,
        StrokeKind::Inside,
    );
    painter.text(
        face.center(),
        egui::Align2::CENTER_CENTER,
        text,
        theme::keycap(),
        colors.text_mid,
    );
    cap.left()
}
