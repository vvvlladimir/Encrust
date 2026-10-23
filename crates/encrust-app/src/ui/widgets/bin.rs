use egui::{Align2, Color32, Id, Rect, Response, Sense, StrokeKind, Ui, vec2};

use crate::i18n::tr;
use crate::ui::theme;

/// How a bin looks while it waits for its first click.
#[derive(Clone, Copy)]
pub enum BinLook {
    /// A bare glyph in the low tone, for a row or beside a field.
    Quiet,
    /// A raised square with the glyph in the danger tone, beside a primary button.
    Framed,
}

/// A bin that asks twice: the first click turns it red, and only a second click on it
/// while red removes. Moving off it puts it back. Answers that second click.
pub fn bin_button(ui: &mut Ui, side: f32, look: BinLook, tooltip: &str, enabled: bool) -> bool {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (_, response) = ui.allocate_exact_size(vec2(side, side), sense);
    confirm_bin(ui, response, look, tooltip, enabled)
}

/// The bin without the second click, where what it takes is cheap to make again.
pub fn instant_bin(ui: &mut Ui, side: f32, look: BinLook, tooltip: &str) -> bool {
    let (_, response) = ui.allocate_exact_size(vec2(side, side), Sense::click());
    paint(ui, response.rect, look, false, response.hovered(), true);
    response.on_hover_text(tooltip).clicked()
}

/// The same bin over a response already laid out, as a row's own icon.
pub fn confirm_bin(
    ui: &Ui,
    response: Response,
    look: BinLook,
    tooltip: &str,
    enabled: bool,
) -> bool {
    let key = Id::new("bin-armed");
    let armed = ui.data(|data| data.get_temp::<Id>(key)) == Some(response.id);
    let fired = enabled && armed && response.clicked();
    if fired || (armed && !response.hovered()) {
        ui.data_mut(|data| data.remove::<Id>(key));
    } else if enabled && response.clicked() {
        ui.data_mut(|data| data.insert_temp(key, response.id));
    }
    let armed = ui.data(|data| data.get_temp::<Id>(key)) == Some(response.id);
    paint(ui, response.rect, look, armed, response.hovered(), enabled);
    let tip = if armed {
        tr!("bin-click-again")
    } else {
        tooltip.to_owned()
    };
    response.on_hover_text(tip);
    fired
}

fn paint(ui: &Ui, rect: Rect, look: BinLook, armed: bool, hovered: bool, enabled: bool) {
    let colors = theme::colors();
    let fill = match (armed, hovered, look) {
        (true, _, _) => colors.danger,
        (false, _, BinLook::Framed) => colors.raised,
        (false, true, BinLook::Quiet) => colors.raised,
        (false, false, BinLook::Quiet) => Color32::TRANSPARENT,
    };
    ui.painter().rect_filled(rect, theme::R_CONTROL, fill);
    if let BinLook::Framed = look {
        let stroke = egui::Stroke::new(1.0, colors.hairline);
        ui.painter()
            .rect_stroke(rect, theme::R_CONTROL, stroke, StrokeKind::Inside);
    }
    let ink = match (enabled, armed, look, hovered) {
        (false, ..) => colors.text_low.gamma_multiply(0.5),
        (true, true, ..) => colors.on_accent,
        (true, false, BinLook::Framed, _) => colors.danger,
        (true, false, BinLook::Quiet, true) => colors.text_high,
        (true, false, BinLook::Quiet, false) => colors.text_low,
    };
    let size_pt = match look {
        BinLook::Quiet => 13.0,
        BinLook::Framed => 18.0,
    };
    // Armed, the bin turns solid and shows a tick, so the second click reads as a yes.
    let glyph = if armed {
        crate::ui::icon::IN_HAND
    } else {
        crate::ui::icon::REMOVE
    };
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        glyph,
        theme::icon(size_pt),
        ink,
    );
}
