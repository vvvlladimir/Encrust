use egui::{Align2, Color32, Rect, Response, Sense, StrokeKind, Ui, vec2};

use super::button::{DISABLED_GLYPH, pressed_rect};
use super::confirm::{ARMED_RING, Arming, arm};
use crate::i18n::tr;
use crate::ui::{icon, theme};

/// How a bin looks while it waits for its first click.
#[derive(Clone, Copy)]
pub enum BinLook {
    /// A bare glyph in the low tone, for a row or beside a field.
    Quiet,
    /// A `companion_square` with the glyph in the danger tone, beside a primary button.
    Framed,
}

/// A bin that asks twice: the first click arms it, and only a second click on it while
/// armed removes. Answers that second click.
pub fn bin_button(ui: &mut Ui, side: f32, look: BinLook, tooltip: &str, enabled: bool) -> bool {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (_, response) = ui.allocate_exact_size(vec2(side, side), sense);
    confirm_bin(ui, response, look, tooltip, enabled)
}

/// The same bin over a response already laid out, as a row's own icon.
pub fn confirm_bin(
    ui: &Ui,
    response: Response,
    look: BinLook,
    tooltip: &str,
    enabled: bool,
) -> bool {
    let Arming { armed, fired } = arm(ui, &response, enabled);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, tooltip));
    paint(ui, &response, look, armed, enabled);
    super::focus_ring(ui, &response, theme::R_CONTROL);
    let tip = if armed {
        tr!("bin-click-again")
    } else {
        tooltip.to_owned()
    };
    response.on_hover_text(tip);
    fired
}

fn paint(ui: &Ui, response: &Response, look: BinLook, armed: bool, enabled: bool) {
    let colors = theme::colors();
    let rect = pressed_rect(ui, response);
    let hovered = enabled && response.hovered();
    let (fill, ring) = match (armed, look, hovered) {
        (true, ..) => (
            colors.danger_wash,
            Some(colors.danger.gamma_multiply(ARMED_RING)),
        ),
        (false, BinLook::Framed, true) => (colors.hover, Some(colors.hairline)),
        (false, BinLook::Framed, false) => (colors.raised, Some(colors.hairline)),
        (false, BinLook::Quiet, true) => (colors.raised, None),
        (false, BinLook::Quiet, false) => (Color32::TRANSPARENT, None),
    };
    paint_box(ui, rect, fill, ring);
    let ink = match (enabled, armed, look, hovered) {
        (false, ..) => colors.text_low.gamma_multiply(DISABLED_GLYPH),
        (true, true, ..) | (true, false, BinLook::Framed, _) => colors.danger,
        (true, false, BinLook::Quiet, true) => colors.text_high,
        (true, false, BinLook::Quiet, false) => colors.text_low,
    };
    let size_pt = match look {
        BinLook::Quiet => 13.0,
        BinLook::Framed if rect.height() >= theme::PRIMARY_H - 2.0 => 18.0,
        BinLook::Framed => 16.0,
    };
    let glyph = if armed { icon::ARMED } else { icon::REMOVE };
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        glyph,
        theme::icon(size_pt),
        ink,
    );
}

fn paint_box(ui: &Ui, rect: Rect, fill: Color32, ring: Option<Color32>) {
    let stroke = ring.map_or(egui::Stroke::NONE, |ring| egui::Stroke::new(1.0, ring));
    ui.painter()
        .rect(rect, theme::R_CONTROL, fill, stroke, StrokeKind::Inside);
}
