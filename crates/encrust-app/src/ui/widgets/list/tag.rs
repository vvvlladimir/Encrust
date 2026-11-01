use egui::{Color32, Id, Response, Sense, Ui, WidgetInfo, WidgetType, vec2};

use crate::ui::theme;
use crate::ui::widgets::{focus_ring, hover_mix};

/// A state named at the end of a row, `right_center` its right end: the word in `ink` on
/// `wash`, a pill. A click on it says more, so it is a button of its own.
pub fn tag(
    ui: &mut Ui,
    right_center: egui::Pos2,
    id: Id,
    text: &str,
    (ink, wash): (Color32, Color32),
) -> Response {
    let word = ui
        .painter()
        .layout_no_wrap(text.to_owned(), theme::small(), ink);
    let size = word.size() + 2.0 * TAG_PAD;
    let rect = egui::Rect::from_min_size(right_center - vec2(size.x, size.y / 2.0), size);
    let response = ui.interact(rect, id, Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, text));
    let fill = hover_mix(ui, &response, wash, ink.gamma_multiply(TAG_HOVER_WASH));
    let painter = ui.painter();
    painter.rect_filled(rect, TAG_R, fill);
    painter.galley(rect.min + TAG_PAD, word, ink);
    focus_ring(ui, &response, TAG_R);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The same pill standing in the flow and taking no press: a heading of release notes.
pub fn tag_label(ui: &mut Ui, text: &str, (ink, wash): (Color32, Color32)) -> Response {
    let word = ui
        .painter()
        .layout_no_wrap(text.to_owned(), theme::small(), ink);
    let (rect, response) = ui.allocate_exact_size(word.size() + 2.0 * TAG_PAD, Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, TAG_R, wash);
    painter.galley(rect.min + TAG_PAD, word, ink);
    response
}

/// One line of a list in prose, after a bullet, wrapping under its own first word.
pub fn bullet(ui: &mut Ui, text: &str) {
    let colors = theme::colors();
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = BULLET_GAP;
        ui.label(
            egui::RichText::new("\u{2022}")
                .font(theme::label())
                .color(colors.text_mid),
        );
        ui.add(
            egui::Label::new(
                egui::RichText::new(text)
                    .font(theme::label())
                    .color(colors.text_high),
            )
            .wrap(),
        );
    });
}

/// Between a bullet and its words, points.
const BULLET_GAP: f32 = 10.0;

/// Room around a tag's word, and how much of its ink washes it under the pointer.
const TAG_PAD: egui::Vec2 = vec2(6.0, 1.0);

const TAG_HOVER_WASH: f32 = 0.24;

const TAG_R: egui::CornerRadius = theme::R_KEY;
