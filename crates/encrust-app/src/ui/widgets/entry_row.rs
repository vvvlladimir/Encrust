use egui::{Align2, Color32, Response, Sense, Ui, vec2};

use super::button::pressed_rect;
use super::{focus_ring, hover_mix, two_lines};
use crate::ui::{icon, theme};

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

/// A model of the models card, one line tall: an eye that shows and hides it, its name cut
/// short with the whole on hover, and at its right what `end` draws.
pub struct ModelRow<'a> {
    pub name: &'a str,
    pub visible: bool,
    pub selected: bool,
    /// The eye's own name for a screen reader: "Show bracket".
    pub eye_name: &'a str,
    /// What tells this model's eye from another's of the same name.
    pub eye_id: egui::Id,
}

/// What a [`model_row`] answers: the row, and the eye at its head.
pub struct ModelPress {
    pub row: Response,
    pub eye: Response,
}

/// Draws a [`ModelRow`]; `end` draws the row's marks leftwards from the right edge it is
/// given and answers where they stop, so the name ends before them.
pub fn model_row(
    ui: &mut Ui,
    row: &ModelRow<'_>,
    end: impl FnOnce(&mut Ui, egui::Rect, f32) -> f32,
) -> ModelPress {
    let colors = theme::colors();
    let size = vec2(ui.available_width(), theme::MODEL_ROW_H);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let (name, selected) = (row.name, row.selected);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, name)
    });
    let fill = match selected {
        true => colors.picked_wash,
        false => hover_mix(ui, &response, Color32::TRANSPARENT, colors.raised),
    };
    ui.painter().rect_filled(rect, 0.0, fill);
    focus_ring(ui, &response, 0.0);
    let eye = model_eye(ui, rect, row);
    let name_right = end(ui, rect, rect.right() - theme::ITEM_GAP) - theme::ITEM_GAP;
    let left = eye.rect.right() + theme::NOTE_GAP;
    model_name(ui, &response, row, (left, name_right));
    ModelPress { row: response, eye }
}

/// The eye at the head of a model's row.
fn model_eye(ui: &mut Ui, rect: egui::Rect, row: &ModelRow<'_>) -> Response {
    let colors = theme::colors();
    let at = egui::pos2(rect.left() + EYE_X, rect.center().y);
    let eye = egui::Rect::from_center_size(at, vec2(EYE_W, rect.height()));
    let hit = eye.shrink2(vec2(0.0, theme::NOTE_GAP));
    let response = ui.interact(hit, row.eye_id, Sense::click());
    let (visible, eye_name) = (row.visible, row.eye_name);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, visible, eye_name)
    });
    focus_ring(ui, &response, theme::R_CONTROL);
    let glyph = if visible { icon::VISIBLE } else { icon::HIDDEN };
    let ink = hover_mix(ui, &response, colors.text_low, colors.text_high);
    let font = theme::icon(theme::glyph::SQUARE);
    ui.painter()
        .text(eye.center(), Align2::CENTER_CENTER, glyph, font, ink);
    let mut response = response;
    response.rect = eye;
    response
}

/// A model's name between `left` and `right`, cut short with its whole on the tooltip.
fn model_name(ui: &Ui, response: &Response, row: &ModelRow<'_>, (left, right): (f32, f32)) {
    let colors = theme::colors();
    let ink = if row.visible {
        colors.text_high
    } else {
        colors.text_low
    };
    // A row is one line tall, so a name longer than it is cut rather than wrapped over
    // the row below it; the whole of it is on the tooltip.
    let mut job =
        egui::text::LayoutJob::simple_singleline(row.name.to_owned(), theme::label(), ink);
    job.wrap = egui::text::TextWrapping::truncate_at_width((right - left).max(0.0));
    let painter = ui.painter();
    let name = painter.layout_job(job);
    let elided = name.elided;
    let at = egui::pos2(left, response.rect.center().y - name.size().y / 2.0);
    painter.galley(at, name, ink);
    if elided {
        response.clone().on_hover_text(row.name);
    }
}

/// Where a model row's eye is centred from its left, and how wide it is, points.
const EYE_X: f32 = 19.0;
const EYE_W: f32 = 18.0;
