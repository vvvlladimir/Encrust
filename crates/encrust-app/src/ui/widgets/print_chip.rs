//! The print chip's parts: a glyph, what is chosen and a caret each, three side by side
//! under one ring; see `docs/design/ui-design-system.md`, Print chip.

use egui::{Align2, Rect, Stroke, pos2};

use super::focus_ring;
use crate::ui::theme;

/// The room either side of what a part says, points.
const PAD: f32 = 10.0;

/// One stretch of the print chip: a glyph, what is chosen, and a caret where it opens a
/// list.
pub struct ChipPart {
    pub glyph: &'static str,
    pub text: std::sync::Arc<egui::Galley>,
    pub caret: &'static str,
    pub width: f32,
    /// The glyph's own colour, lit or not: the resin's.
    pub glyph_tint: Option<egui::Color32>,
}

impl ChipPart {
    /// A part whose text is cut short with an ellipsis past `max_text_w` points.
    pub fn new(
        ui: &egui::Ui,
        glyph: &'static str,
        text: &str,
        tint: egui::Color32,
        caret: &'static str,
        max_text_w: f32,
    ) -> Self {
        let whole = ui
            .painter()
            .layout_no_wrap(text.to_owned(), theme::label(), tint);
        // Laid out again only when cut: a truncating layout sets even a whole text apart.
        let text = if whole.size().x <= max_text_w {
            whole
        } else {
            let mut job =
                egui::text::LayoutJob::simple_singleline(text.to_owned(), theme::label(), tint);
            job.wrap = egui::text::TextWrapping::truncate_at_width(max_text_w.max(0.0));
            ui.painter().layout_job(job)
        };
        let width = Self::chrome_w(caret) + text.size().x;
        Self {
            glyph,
            text,
            caret,
            width,
            glyph_tint: None,
        }
    }

    /// The same part with its glyph in `tint`.
    pub fn glyph_tint(self, tint: egui::Color32) -> Self {
        Self {
            glyph_tint: Some(tint),
            ..self
        }
    }

    /// The part's width less its text: the room either side, the glyph and the caret.
    pub fn chrome_w(caret: &str) -> f32 {
        let caret_w = if caret.is_empty() { 0.0 } else { 16.0 };
        PAD + 20.0 + caret_w + PAD
    }

    /// Paints the part in `zone`, lit while what it opens is open.
    fn paint(
        self,
        painter: &egui::Painter,
        zone: Rect,
        radius: egui::CornerRadius,
        lit: bool,
        hovered: bool,
    ) {
        let colors = theme::colors();
        if lit {
            painter.rect_filled(zone, radius, colors.accent_wash);
        } else if hovered {
            painter.rect_filled(zone, radius, colors.hover);
        }
        let glyph_tint = match (self.glyph_tint, lit) {
            (Some(tint), _) => tint,
            (None, true) => colors.accent_soft,
            (None, false) => colors.text_low,
        };
        painter.text(
            pos2(zone.left() + PAD + 7.0, zone.center().y),
            Align2::CENTER_CENTER,
            self.glyph,
            theme::icon(14.0),
            glyph_tint,
        );
        let at = pos2(
            zone.left() + PAD + 20.0,
            zone.center().y - self.text.size().y / 2.0,
        );
        let text_w = self.text.size().x;
        let fallback = if lit {
            colors.accent_soft
        } else {
            colors.text_high
        };
        painter.galley(at, self.text, fallback);
        if !self.caret.is_empty() {
            painter.text(
                pos2(at.x + text_w + 9.0, zone.center().y),
                Align2::CENTER_CENTER,
                self.caret,
                theme::icon(11.0),
                colors.text_low,
            );
        }
    }
}

/// Paints the three parts in `zones` of the chip's `rect`, each lit while what it opens is
/// `open`, with a rule between each two.
pub fn paint_chip(
    ui: &egui::Ui,
    parts: [ChipPart; 3],
    zones: [Rect; 3],
    rect: Rect,
    responses: &[egui::Response; 3],
    open: [bool; 3],
) {
    let colors = theme::colors();
    let painter = ui.painter();
    for (index, (part, zone)) in parts.into_iter().zip(zones).enumerate() {
        // Square where a part meets the rule beside it.
        let mut radius = theme::R_CONTROL;
        if index > 0 {
            (radius.nw, radius.sw) = (0, 0);
        }
        if index < 2 {
            (radius.ne, radius.se) = (0, 0);
        }
        part.paint(
            painter,
            zone,
            radius,
            open[index],
            responses[index].hovered(),
        );
    }
    for zone in &zones[1..] {
        painter.vline(
            zone.left(),
            rect.y_range(),
            Stroke::new(1.0, colors.hairline),
        );
    }
    let stroke = Stroke::new(1.0, colors.hairline);
    painter.rect_stroke(rect, theme::R_CONTROL, stroke, egui::StrokeKind::Inside);
    for response in responses {
        focus_ring(ui, response, theme::R_CONTROL);
    }
}

/// The width `text` takes in the chip, uncut.
pub fn chip_text_w(ui: &egui::Ui, text: &str) -> f32 {
    ui.painter()
        .layout_no_wrap(text.to_owned(), theme::label(), egui::Color32::WHITE)
        .size()
        .x
}
