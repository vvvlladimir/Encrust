//! Every list that drops from a box: the box that says what is chosen, the card the list
//! hangs in, and a row of it. See `docs/design/ui-design-system.md`, Dropdown.

use egui::{
    Align2, Color32, PopupCloseBehavior, Rect, Response, Sense, StrokeKind, Ui, WidgetInfo,
    WidgetType,
};
use egui::{pos2, vec2};

use super::button::sense_for;
use super::chooser::{Lead, chooser_frame};
use super::{focus_ring, hover_mix};
use crate::i18n::tr;
use crate::ui::{icon, theme};

/// What a closed dropdown shows: what leads it, what is chosen, `· edited` in `warn` once
/// the values differ from the ones it names, and a count before the caret.
pub struct DropdownFace<'a> {
    pub lead: Option<Lead<'a>>,
    pub label: &'a str,
    pub edited: bool,
    pub count: Option<usize>,
    /// The list holds nothing: no caret, and a press opens nothing.
    pub empty: bool,
}

impl<'a> DropdownFace<'a> {
    /// A face with no more than a glyph and what is chosen.
    pub fn plain(glyph: &'a str, label: &'a str) -> Self {
        Self {
            lead: Some(Lead::Glyph(glyph)),
            label,
            edited: false,
            count: None,
            empty: false,
        }
    }

    /// The same face over a list that holds nothing while `empty`.
    pub fn empty(self, empty: bool) -> Self {
        Self { empty, ..self }
    }
}

/// The closed dropdown, `width` points wide: a box on `raised` holding `face` and the
/// caret. Open its list with [`dropdown_menu`].
pub fn dropdown(ui: &mut Ui, face: &DropdownFace<'_>, width: f32) -> Response {
    let colors = theme::colors();
    let size = vec2(width, theme::FIELD_H);
    let (rect, response) = ui.allocate_exact_size(size, sense_for(!face.empty));
    let enabled = ui.is_enabled();
    response.widget_info(|| WidgetInfo::labeled(WidgetType::ComboBox, enabled, face.label));
    let border = hover_mix(ui, &response, colors.hairline, colors.line);
    let painter = ui.painter();
    painter.rect(
        rect,
        theme::R_CONTROL,
        colors.raised,
        egui::Stroke::new(1.0, border),
        StrokeKind::Inside,
    );
    let right = caret_and_count(painter, rect, face);
    let left = match &face.lead {
        Some(lead) => {
            lead_at(
                painter,
                pos2(rect.left() + PAD + GLYPH_W / 2.0, rect.center().y),
                lead,
            );
            rect.left() + PAD + GLYPH_W + GAP
        }
        None => rect.left() + PAD,
    };
    label(
        painter,
        Rect::from_x_y_ranges(left..=right, rect.y_range()),
        face,
    );
    focus_ring(ui, &response, theme::R_CONTROL);
    response
}

/// The caret, unless there is nothing to choose, and the count before it; answers where
/// the label must stop.
fn caret_and_count(painter: &egui::Painter, rect: Rect, face: &DropdownFace<'_>) -> f32 {
    let low = theme::colors().text_low;
    let mut right = rect.right() - PAD;
    if !face.empty {
        let at = pos2(rect.right() - PAD - GLYPH_W / 2.0, rect.center().y);
        let caret = painter.text(
            at,
            Align2::CENTER_CENTER,
            icon::CARET_DOWN,
            theme::icon(14.0),
            low,
        );
        right = caret.left() - GAP;
    }
    if let Some(count) = face.count {
        let at = pos2(right, rect.center().y);
        let figure = painter.text(
            at,
            Align2::RIGHT_CENTER,
            count.to_string(),
            theme::figures(12.0),
            low,
        );
        right = figure.left() - GAP;
    }
    right
}

/// What is chosen, and `· edited` after it, cut to `room`.
fn label(painter: &egui::Painter, room: Rect, face: &DropdownFace<'_>) {
    let colors = theme::colors();
    let mut job = egui::text::LayoutJob::default();
    let format = |color| egui::TextFormat::simple(theme::label(), color);
    job.append(face.label, 0.0, format(colors.text_high));
    if face.edited {
        job.append(&tr!("presets-edited"), 4.0, format(colors.warn));
    }
    job.wrap = egui::text::TextWrapping::truncate_at_width(room.width().max(0.0));
    let galley = painter.layout_job(job);
    let top = room.center().y - galley.size().y / 2.0;
    painter.galley(pos2(room.left(), top), galley, colors.text_high);
}

fn lead_at(painter: &egui::Painter, at: egui::Pos2, lead: &Lead<'_>) {
    match lead {
        Lead::Glyph(glyph) => {
            painter.text(
                at,
                Align2::CENTER_CENTER,
                *glyph,
                theme::icon(14.0),
                theme::colors().text_low,
            );
        }
        Lead::Dot(tint) => {
            painter.circle_filled(at, DOT_R, *tint);
        }
    }
}

/// The list under `response`, as wide as the box, in the card every dropdown hangs in.
pub fn dropdown_menu<R>(response: &Response, add: impl FnOnce(&mut Ui) -> R) -> Option<R> {
    menu(response, PopupCloseBehavior::CloseOnClick, add)
}

/// A [`dropdown_menu`] that a click inside leaves open, for a list headed by a field to
/// type into; a row chosen closes it with `ui.close()`.
pub fn dropdown_search_menu<R>(response: &Response, add: impl FnOnce(&mut Ui) -> R) -> Option<R> {
    menu(response, PopupCloseBehavior::CloseOnClickOutside, add)
}

fn menu<R>(
    response: &Response,
    close: PopupCloseBehavior,
    add: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    egui::Popup::menu(response)
        .gap(MENU_GAP)
        .close_behavior(close)
        .width(response.rect.width())
        .frame(chooser_frame())
        .show(|ui| {
            ui.set_min_width(response.rect.width() - 2.0 * theme::TOOLBAR_PAD);
            ui.spacing_mut().item_spacing.y = 1.0;
            add(ui)
        })
        .map(|shown| shown.inner)
}

/// One choice of a dropdown's list: what leads it, its name, a figure at its right, and
/// while it is the one chosen a wash and a tick.
pub struct DropdownRow<'a> {
    pub lead: Option<Lead<'a>>,
    pub name: &'a str,
    pub aside: Option<&'a str>,
    pub chosen: bool,
}

impl<'a> DropdownRow<'a> {
    /// A row of no more than its name.
    pub fn named(name: &'a str, chosen: bool) -> Self {
        Self {
            lead: None,
            name,
            aside: None,
            chosen,
        }
    }
}

/// Draws a [`DropdownRow`], as tall as the box it drops from.
pub fn dropdown_row(ui: &mut Ui, row: &DropdownRow<'_>) -> Response {
    let colors = theme::colors();
    let size = vec2(ui.available_width(), theme::FIELD_H);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| {
        WidgetInfo::selected(WidgetType::SelectableLabel, true, row.chosen, row.name)
    });
    let fill = match row.chosen {
        true => colors.picked_wash,
        false => hover_mix(ui, &response, Color32::TRANSPARENT, colors.hover),
    };
    let painter = ui.painter();
    painter.rect_filled(rect, theme::R_CONTROL, fill);
    let lead_x = rect.left() + PAD + GLYPH_W / 2.0;
    if let Some(lead) = &row.lead {
        lead_at(painter, pos2(lead_x, rect.center().y), lead);
    }
    let name_x = lead_x + GLYPH_W / 2.0 + GAP;
    painter.text(
        pos2(name_x, rect.center().y),
        Align2::LEFT_CENTER,
        row.name,
        theme::label(),
        colors.text_high,
    );
    let tick_x = rect.right() - PAD - GLYPH_W / 2.0;
    if row.chosen {
        painter.text(
            pos2(tick_x, rect.center().y),
            Align2::CENTER_CENTER,
            icon::IN_HAND,
            theme::icon(14.0),
            colors.accent_soft,
        );
    }
    if let Some(aside) = row.aside {
        painter.text(
            pos2(tick_x - GLYPH_W, rect.center().y),
            Align2::RIGHT_CENTER,
            aside,
            theme::figures(12.0),
            colors.text_low,
        );
    }
    focus_ring(ui, &response, theme::R_CONTROL);
    response
}

/// Room at a box's ends, the square a glyph stands in and the room after it, points; and a
/// colour's dot.
const PAD: f32 = 10.0;
const GLYPH_W: f32 = 14.0;
const GAP: f32 = 8.0;
const DOT_R: f32 = 4.5;
/// Between a box and the list that drops from it, points.
pub const MENU_GAP: f32 = 6.0;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::widgets::fixtures;

    #[test]
    fn a_dropdown_and_its_rows_stand_as_tall_as_a_field() {
        let ctx = fixtures::context();
        let (mut closed, mut row) = (0.0, 0.0);
        fixtures::frame(&ctx, egui::RawInput::default(), |ui| {
            let face = DropdownFace::plain(icon::PRESET, "Medium");
            closed = dropdown(ui, &face, 200.0).rect.height();
            row = dropdown_row(ui, &DropdownRow::named("Light", true))
                .rect
                .height();
        });
        assert_eq!((closed, row), (theme::FIELD_H, theme::FIELD_H));
    }

    #[test]
    fn an_empty_dropdown_takes_no_press() {
        let ctx = fixtures::context();
        let mut senses = Vec::new();
        fixtures::frame(&ctx, egui::RawInput::default(), |ui| {
            for empty in [false, true] {
                let face = DropdownFace::plain(icon::PRESET, "None").empty(empty);
                senses.push(dropdown(ui, &face, 200.0).sense.senses_click());
            }
        });
        assert_eq!(senses, [true, false]);
    }
}
