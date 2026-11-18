//! A run told in the box of the picker that started it: what it is doing and how far along,
//! or why it waits. See `docs/design/ui-screens.md`, Prepare row.

use egui::{Align2, Rect, Response, Sense, StrokeKind, Ui, WidgetInfo, WidgetType, pos2, vec2};

use super::dropdown::{GAP, GLYPH_W, PAD, cut_to};
use super::over_stage::paint_spinner;
use crate::i18n::tr;
use crate::ui::{icon, theme};

/// Where a run stands, as its box says it.
pub enum RunState {
    /// Going: its step turns the glyph.
    Going,
    /// Waiting at a pause for Continue.
    Paused,
    /// Waiting at a step that failed.
    Failed,
}

/// The box a run stands in, `width` points wide and as tall as the picker it took the place
/// of: washed in the accent to `share`, the state's glyph, `what` cut to fit, and the share
/// as a percent. The whole of `what` on hover.
pub fn run_box(ui: &mut Ui, state: &RunState, what: &str, share: f32, width: f32) -> Response {
    let colors = theme::colors();
    let (rect, response) = ui.allocate_exact_size(vec2(width, theme::FIELD_H), Sense::hover());
    response.widget_info(|| {
        let mut info = WidgetInfo::labeled(WidgetType::ProgressIndicator, true, what);
        info.value = Some(f64::from(share));
        info
    });
    let painter = ui.painter();
    painter.rect_filled(rect, theme::R_CONTROL, colors.raised);
    let done = rect.with_max_x(rect.left() + rect.width() * share.clamp(0.0, 1.0));
    painter.rect_filled(done, theme::R_CONTROL, colors.accent_wash);
    let border = match state {
        RunState::Failed => colors.danger,
        _ => colors.hairline,
    };
    painter.rect_stroke(
        rect,
        theme::R_CONTROL,
        egui::Stroke::new(1.0, border),
        StrokeKind::Inside,
    );
    let lead_at = pos2(rect.left() + PAD + GLYPH_W / 2.0, rect.center().y);
    let still = match state {
        RunState::Going => None,
        RunState::Paused => Some((icon::PAUSE, colors.accent_soft)),
        RunState::Failed => Some((icon::WARNING, colors.danger)),
    };
    match still {
        None => paint_spinner(ui, lead_at),
        Some((glyph, tint)) => {
            let font = theme::icon(theme::glyph::ROW);
            painter.text(lead_at, Align2::CENTER_CENTER, glyph, font, tint);
        }
    }
    let percent = tr!("value-percent", value = format!("{:.0}", share * 100.0));
    let figure = painter.text(
        pos2(rect.right() - PAD, rect.center().y),
        Align2::RIGHT_CENTER,
        percent,
        theme::figures(12.0),
        colors.text_mid,
    );
    let left = rect.left() + PAD + GLYPH_W + GAP;
    let room = Rect::from_x_y_ranges(left..=figure.left() - GAP, rect.y_range());
    let words = cut_to(painter, what, room.width().max(0.0), colors.text_high);
    let top = rect.center().y - words.size().y / 2.0;
    painter.galley(pos2(left, top), words, colors.text_high);
    response.on_hover_text(what)
}
