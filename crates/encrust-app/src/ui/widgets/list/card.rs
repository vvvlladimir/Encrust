//! A kind of issue as one card: a heading that opens it, its rows and what answers them, or
//! the one line of what answered it.

use egui::{Align2, Color32, Frame, Rect, Response, Sense, Ui, WidgetInfo, WidgetType, vec2};

use crate::ui::theme::motion;
use crate::ui::widgets::button::small_icon_button;
use crate::ui::widgets::dropdown::cut_to;
use crate::ui::widgets::figures::{Grows, unfold};
use crate::ui::widgets::{focus_ring, hover_mix};
use crate::ui::{icon, theme};

/// Height of a row inside a card's list, points.
const LINE_H: f32 = 30.0;
/// Rows a card's list shows before the rest scroll inside it.
const LINES_SHOWN: f32 = 5.0;
const LINE_GAP: f32 = 1.0;
/// Inner padding of a card's list and of its open body, points.
const LIST_PAD: i8 = 2;
const BODY_PAD: i8 = 6;
/// Room at a heading's ends and between its parts, points.
const END: f32 = 10.0;
const GAP: f32 = 8.0;
/// How strongly the accent rings the row in hand.
const LINE_RING: f32 = 0.35;

/// What a card's heading says.
pub struct IssueHead<'a> {
    pub glyph: &'a str,
    pub tint: Color32,
    pub name: &'a str,
    /// Where they lie, or why they are left to the hand; quiet, cut first.
    pub aside: &'a str,
    pub count: usize,
}

fn card_frame() -> Frame {
    Frame::new()
        .fill(theme::colors().raised)
        .corner_radius(theme::R_CONTROL)
}

/// A kind of issue: its heading, and under it while `open` what `body` adds. Answers whether
/// the heading was pressed, and what the body answered.
pub fn issue_card<R>(
    ui: &mut Ui,
    head: &IssueHead<'_>,
    open: bool,
    body: impl FnOnce(&mut Ui) -> R,
) -> (bool, Option<R>) {
    let id = ui.id().with(("issue-card", head.name));
    let shown = card_frame().show(ui, |ui| {
        // Scoped by the card, as two are open at once while one folds and the other unfolds.
        ui.push_id(head.name, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let pressed = heading(ui, head, open).clicked();
            let how_open = motion::fold(ui.ctx(), id.with("fold"), open);
            let inner = (how_open > 0.0).then(|| {
                unfold(ui, id, how_open, Grows::Down, |ui| {
                    let margin = egui::Margin {
                        left: BODY_PAD,
                        right: BODY_PAD,
                        top: 0,
                        bottom: BODY_PAD,
                    };
                    Frame::new()
                        .inner_margin(margin)
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = theme::ROW_GAP;
                            body(ui)
                        })
                        .inner
                })
            });
            (pressed, inner)
        })
        .inner
    });
    shown.inner
}

fn heading(ui: &mut Ui, head: &IssueHead<'_>, open: bool) -> Response {
    let colors = theme::colors();
    let size = vec2(ui.available_width(), theme::BUTTON_H);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response
        .widget_info(|| WidgetInfo::selected(WidgetType::CollapsingHeader, true, open, head.name));
    let fill = hover_mix(ui, &response, colors.raised, colors.hover);
    ui.painter().rect_filled(rect, theme::R_CONTROL, fill);
    paint_heading(ui, rect, head, open);
    focus_ring(ui, &response, theme::R_CONTROL);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn paint_heading(ui: &Ui, rect: Rect, head: &IssueHead<'_>, open: bool) {
    let colors = theme::colors();
    let painter = ui.painter();
    let lead = rect.left_center() + vec2(END, 0.0);
    let glyph = theme::icon(theme::glyph::SQUARE);
    let drawn = painter.text(lead, Align2::LEFT_CENTER, head.glyph, glyph, head.tint);
    let caret = if open {
        icon::CARET_DOWN
    } else {
        icon::CARET_RIGHT
    };
    let font = theme::icon(theme::glyph::CARET);
    let at = rect.right_center() - vec2(END, 0.0);
    let tail = painter.text(at, Align2::RIGHT_CENTER, caret, font, colors.text_low);
    let at = tail.left_center() - vec2(GAP, 0.0);
    let figure = theme::figures(12.0);
    let count = head.count.to_string();
    let counted = painter.text(at, Align2::RIGHT_CENTER, count, figure, colors.text_mid);

    let left = drawn.right() + GAP;
    let room = (counted.left() - GAP - left).max(0.0);
    let name = cut_to(painter, head.name, room, colors.text_high);
    let name_at = egui::pos2(left, rect.center().y - name.size().y / 2.0);
    let name_right = left + name.size().x;
    painter.galley(name_at, name, colors.text_high);
    if !open {
        let room = (counted.left() - GAP - name_right - GAP).max(0.0);
        right_words(
            painter,
            (counted.left() - GAP, rect),
            head.aside,
            (room, colors.text_low),
        );
    }
}

/// `words`, small, in `ink`, cut to `room` and standing with their right end at `right`.
fn right_words(
    painter: &egui::Painter,
    (right, rect): (f32, Rect),
    words: &str,
    (room, ink): (f32, Color32),
) {
    let mut job = egui::text::LayoutJob::simple_singleline(words.to_owned(), theme::small(), ink);
    job.wrap = egui::text::TextWrapping::truncate_at_width(room);
    let galley = painter.layout_job(job);
    let at = egui::pos2(
        right - galley.size().x,
        rect.center().y - galley.size().y / 2.0,
    );
    painter.galley(at, galley, ink);
}

/// A card's rows on `sunken`, past [`LINES_SHOWN`] scrolling inside it.
pub fn issue_lines<R>(ui: &mut Ui, name: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    let height = LINES_SHOWN * LINE_H + (LINES_SHOWN - 1.0) * LINE_GAP;
    Frame::new()
        .fill(theme::colors().sunken)
        .corner_radius(theme::R_CONTROL)
        .inner_margin(LIST_PAD)
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt(("issue-lines", name))
                .max_height(height)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = LINE_GAP;
                    add(ui)
                })
                .inner
        })
        .inner
}

/// One row of a card's list: where it is, and `detail` at the right in `ink`. The whole row
/// is the button that goes there.
pub fn issue_line(
    ui: &mut Ui,
    place: &str,
    (detail, ink): (&str, Color32),
    selected: bool,
) -> Response {
    let colors = theme::colors();
    let size = vec2(ui.available_width(), LINE_H);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| {
        WidgetInfo::selected(
            WidgetType::SelectableLabel,
            true,
            selected,
            format!("{place}, {detail}"),
        )
    });
    let fill = match selected {
        true => colors.picked_wash,
        false => hover_mix(ui, &response, colors.sunken, colors.hover),
    };
    let ring = match selected {
        true => egui::Stroke::new(1.0, colors.accent.gamma_multiply(LINE_RING)),
        false => egui::Stroke::NONE,
    };
    let inside = egui::StrokeKind::Inside;
    ui.painter()
        .rect(rect, theme::R_CONTROL, fill, ring, inside);
    paint_line(ui, rect, place, (detail, ink));
    focus_ring(ui, &response, theme::R_CONTROL);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn paint_line(ui: &Ui, rect: Rect, place: &str, (detail, ink): (&str, Color32)) {
    let colors = theme::colors();
    let painter = ui.painter();
    let room = rect.width() - 2.0 * GAP;
    let figures = theme::figures(12.0);
    let mut job = egui::text::LayoutJob::simple_singleline(detail.to_owned(), figures, ink);
    job.wrap = egui::text::TextWrapping::truncate_at_width(room * 0.7);
    let said = painter.layout_job(job);
    let at = egui::pos2(
        rect.right() - GAP - said.size().x,
        rect.center().y - said.size().y / 2.0,
    );
    let left_of_detail = at.x;
    painter.galley(at, said, ink);
    let name = cut_to(
        painter,
        place,
        left_of_detail - GAP - rect.left() - GAP,
        colors.text_high,
    );
    let at = egui::pos2(rect.left() + GAP, rect.center().y - name.size().y / 2.0);
    painter.galley(at, name, colors.text_high);
}

/// A kind of issue already answered: a tick, its name, what was done, read whole on hover,
/// and Undo where `undo` names it. Answers whether Undo was pressed.
pub fn answered_row(ui: &mut Ui, name: &str, said: &str, undo: Option<&str>) -> bool {
    let colors = theme::colors();
    let size = vec2(ui.available_width(), theme::BUTTON_H);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, theme::R_CONTROL, colors.raised);
    let lead = rect.left_center() + vec2(END, 0.0);
    let glyph = theme::icon(theme::glyph::SQUARE);
    let drawn = painter.text(lead, Align2::LEFT_CENTER, icon::CLEAR, glyph, colors.ok);
    let label = theme::label();
    let at = drawn.right_center() + vec2(GAP, 0.0);
    let named = painter.text(at, Align2::LEFT_CENTER, name, label, colors.text_mid);

    let square = rect.right_center() - vec2(3.0, 0.0);
    let right = match undo {
        Some(_) => square.x - theme::BUTTON_H,
        None => rect.right() - END,
    };
    let room = (right - named.right() - GAP).max(0.0);
    right_words(painter, (right, rect), said, (room, colors.text_mid));
    response.on_hover_text(said);
    undo.is_some_and(|tip| {
        let place = Rect::from_center_size(
            egui::pos2(rect.right() - 16.0, rect.center().y),
            vec2(26.0, 26.0),
        );
        ui.put(place, |ui: &mut Ui| small_icon_button(ui, icon::UNDO, tip))
            .clicked()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::widgets::fixtures::clashes;

    #[test]
    fn two_open_cards_listing_under_one_name_never_share_an_id() {
        let two_cards = |ui: &mut Ui| {
            for name in ["Islands", "Suction cups"] {
                let head = IssueHead {
                    glyph: icon::WARNING,
                    tint: Color32::RED,
                    name,
                    aside: "",
                    count: 1,
                };
                issue_card(ui, &head, true, |ui| {
                    issue_lines(ui, "layers", |ui| {
                        issue_line(ui, "Layer 1", ("an island", Color32::GRAY), false);
                    });
                });
            }
        };
        assert!(!clashes(two_cards), "egui warns of an id used twice");
    }
}
