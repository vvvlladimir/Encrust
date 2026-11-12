//! Why a carried support may not stand where the cursor puts it, beside the cursor (ADR 0437).

use core_supports::Refused;

use crate::i18n::tr;
use crate::panels::Window;
use crate::ui::theme;

use super::bounds::figure_plate;

/// Where the reason stands from the cursor, points: below and to the right of the pointer.
const REASON_OFFSET: egui::Vec2 = egui::vec2(36.0, 22.0);

/// The reason the support in hand is refused, if it is.
pub(super) fn draw_refusal(ui: &egui::Ui, window: &Window, viewport: egui::Rect) {
    let Some(why) = window
        .tools
        .supports
        .carry
        .as_ref()
        .and_then(|carry| carry.refused)
    else {
        return;
    };
    if let Some(cursor) = ui.input(|input| input.pointer.latest_pos()) {
        let painter = ui.painter_at(viewport);
        figure_plate(
            &painter,
            cursor + REASON_OFFSET,
            refusal_reason(why),
            theme::colors().danger,
        );
    }
}

/// What `why` says to the hand.
fn refusal_reason(why: Refused) -> String {
    match why {
        Refused::Collides => tr!("supports-refused-collides"),
        Refused::Leans => tr!("supports-refused-leans"),
        Refused::NoFooting => tr!("supports-refused-no-footing"),
        Refused::NoHold => tr!("supports-refused-no-hold"),
    }
}
