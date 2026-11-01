use egui::{Align2, Color32, Response, Sense, Ui, vec2};

use super::button::pressed_rect;
use super::{focus_ring, hover_mix, two_lines};
use crate::ui::theme;

/// One entry of a window's list, two lines tall: its name over a detail, and quiet glyphs
/// in their tints at its right, right to left. Washed as picked while `picked`.
pub struct EntryRow<'a> {
    pub name: &'a str,
    pub detail: &'a str,
    pub marks: &'a [(&'a str, Color32)],
    pub picked: bool,
}

/// Draws an [`EntryRow`] across the room it is given.
pub fn entry_row(ui: &mut Ui, row: &EntryRow<'_>) -> Response {
    let colors = theme::colors();
    let size = vec2(ui.available_width(), theme::TWO_LINE_ROW_H);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let (name, picked) = (row.name, row.picked);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, picked, name)
    });
    let fill = match picked {
        true => colors.picked_wash,
        false => hover_mix(ui, &response, Color32::TRANSPARENT, colors.hover),
    };
    let painter = ui.painter_at(rect);
    painter.rect_filled(pressed_rect(ui, &response), 0.0, fill);
    two_lines(
        &painter,
        rect.left_center() + vec2(ENTRY_INSET, 0.0),
        name,
        row.detail,
    );
    let mut right = rect.right_center() - vec2(ENTRY_MARKS_INSET, 0.0);
    for &(glyph, tint) in row.marks {
        let font = theme::icon(theme::glyph::ASIDE);
        painter.text(right, Align2::RIGHT_CENTER, glyph, font, tint);
        right.x -= ENTRY_MARK_STEP;
    }
    focus_ring(ui, &response, 0.0);
    response
}

/// Where an entry's lines start from its left, its first mark from its right, and the step
/// from one mark to the next.
const ENTRY_INSET: f32 = 14.0;
const ENTRY_MARKS_INSET: f32 = 16.0;
const ENTRY_MARK_STEP: f32 = 22.0;
