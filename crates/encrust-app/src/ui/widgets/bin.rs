use egui::{Align2, Color32, Rect, Response, Sense, StrokeKind, Ui, vec2};

use super::button::{DISABLED_GLYPH, hover_mix, pressed_rect};
use super::confirm::{Arming, arm, armed_ring};
use crate::i18n::tr;
use crate::ui::{icon, theme};

/// How a bin looks while it waits for its first click.
#[derive(Clone, Copy)]
pub enum BinLook {
    /// A bare glyph in the low tone, for a row or beside a field.
    Quiet,
    /// A `companion_square`, beside a primary button.
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
    let response = response.on_hover_text(tooltip);
    if armed {
        response.on_hover_text(tr!("bin-click-again"));
    }
    fired
}

fn paint(ui: &Ui, response: &Response, look: BinLook, armed: bool, enabled: bool) {
    let colors = theme::colors();
    let rect = pressed_rect(ui, response);
    let hovered = enabled && response.hovered();
    let (fill, ring) = match (armed, look, hovered) {
        (true, ..) => (colors.danger_wash, Some(armed_ring(ui.ctx()))),
        (false, BinLook::Framed, _) => (
            eased(ui, response, enabled, colors.raised, colors.hover),
            Some(colors.hairline),
        ),
        (false, BinLook::Quiet, _) => (
            eased(ui, response, enabled, Color32::TRANSPARENT, colors.raised),
            None,
        ),
    };
    paint_box(ui, rect, fill, ring);
    let ink = match (enabled, armed, look, hovered) {
        (false, ..) => colors.text_low.gamma_multiply(DISABLED_GLYPH),
        (true, true, ..) => colors.danger,
        (true, false, _, true) => colors.text_high,
        (true, false, BinLook::Framed, false) => colors.text_mid,
        (true, false, BinLook::Quiet, false) if rect.height() >= theme::BUTTON_H - 2.0 => {
            colors.text_mid
        }
        (true, false, BinLook::Quiet, false) => colors.text_low,
    };
    let size_pt = match look {
        BinLook::Quiet if rect.height() >= theme::BUTTON_H - 2.0 => 16.0,
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

/// `rest`, easing to `hovered` under the pointer while the bin can be used.
fn eased(ui: &Ui, response: &Response, enabled: bool, rest: Color32, hovered: Color32) -> Color32 {
    match enabled {
        true => hover_mix(ui, response, rest, hovered),
        false => rest,
    }
}

fn paint_box(ui: &Ui, rect: Rect, fill: Color32, ring: Option<Color32>) {
    let stroke = ring.map_or(egui::Stroke::NONE, |ring| egui::Stroke::new(1.0, ring));
    ui.painter()
        .rect(rect, theme::R_CONTROL, fill, stroke, StrokeKind::Inside);
}
