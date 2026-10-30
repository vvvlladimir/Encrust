use egui::{Painter, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

use crate::ui::theme;

/// Room either side of a key's face, and the cap's height; its lower edge is drawn thicker.
const CAP_PAD_X: f32 = 5.0;
const CAP_H: f32 = 18.0;
const CAP_MIN_W: f32 = 18.0;

/// The width a keycap saying `text` takes.
pub fn keycap_width(painter: &Painter, text: &str) -> f32 {
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

/// The keys that work a window, at its foot: each set of caps then what they do in
/// `text_low`, left to right on one row of `height` points.
pub fn key_row(ui: &mut Ui, keys: &[(&[&str], &str)], height: f32) {
    let colors = theme::colors();
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let painter = ui.painter();
    let y = rect.center().y;
    let mut x = rect.left();
    for (caps, what) in keys {
        for cap in *caps {
            let width = keycap_width(painter, cap);
            keycap(painter, cap, x + width, y);
            x += width + KEY_GAP;
        }
        let words = painter.text(
            pos2(x + 2.0, y),
            egui::Align2::LEFT_CENTER,
            *what,
            theme::small(),
            colors.text_low,
        );
        x = words.right() + PAIR_GAP;
    }
}

/// Room between two caps of a chord, and between one key's words and the next key.
const KEY_GAP: f32 = 3.0;
const PAIR_GAP: f32 = 16.0;
