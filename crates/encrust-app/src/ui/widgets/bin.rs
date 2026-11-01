use egui::{Response, Ui, vec2};

use super::button::{SquareLook, paint_square, sense_for};
use super::confirm::{Arming, arm, armed_ring};
use crate::i18n::tr;
use crate::ui::{icon, theme};

/// A bin that asks twice: the first click arms it, and only a second click on it while
/// armed removes. Answers that second click.
pub fn bin_button(ui: &mut Ui, side: f32, look: SquareLook, tooltip: &str, enabled: bool) -> bool {
    let sense = sense_for(enabled);
    let (_, response) = ui.allocate_exact_size(vec2(side, side), sense);
    confirm_bin(ui, response, look, tooltip, enabled)
}

/// The same bin over a response already laid out, as a row's own icon.
pub fn confirm_bin(
    ui: &Ui,
    response: Response,
    look: SquareLook,
    tooltip: &str,
    enabled: bool,
) -> bool {
    let Arming { armed, fired } = arm(ui, &response, enabled);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, tooltip));
    let glyph = if armed { icon::ARMED } else { icon::REMOVE };
    let ring = armed.then(|| armed_ring(ui.ctx()));
    paint_square(ui, &response, (look, glyph), ring, enabled);
    super::focus_ring(ui, &response, theme::R_CONTROL);
    let response = response.on_hover_text(tooltip);
    if armed {
        response.on_hover_text(tr!("bin-click-again"));
    }
    fired
}
