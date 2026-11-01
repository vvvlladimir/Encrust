//! The two boxes a list is narrowed by: a window's find line and Find's palette field.

use egui::{Align, Align2, Id, Key, Modifiers, Response, Sense, Stroke, TextEdit, Ui, pos2, vec2};

use super::field::text_box;
use super::keycap::{keycap, keycap_width};
use super::{focus_ring, hover_mix};
use crate::ui::{icon, theme};

/// From a find line's right edge to its keycap's, points.
const FIND_CAP_INSET: f32 = 6.0;
/// The palette field's inset from its edges, points.
const PALETTE_INSET: i8 = 16;

/// The line that narrows a window's list, `⌘F` reaching it from anywhere in the window.
pub fn find_line(ui: &mut Ui, find: &mut String, hint: &str) {
    let keys = format!("{}F", icon::KEY_COMMAND);
    let field = ui.add(
        text_box(TextEdit::singleline(find))
            .hint_text(format!("{}  {hint}", icon::FIND))
            .desired_width(ui.available_width()),
    );
    if ui.input_mut(|input| input.consume_key(Modifiers::COMMAND, Key::F)) {
        field.request_focus();
    }
    if find.is_empty() {
        let right = field.rect.right() - FIND_CAP_INSET;
        keycap(ui.painter(), &keys, right, field.rect.center().y);
    }
}

/// The field a palette is typed in, `width` by `height` points over a hairline, holding the
/// focus while it is up and carrying `close` as a keycap. Answers whether the text changed.
pub fn palette_field(
    ui: &mut Ui,
    id: Id,
    query: &mut String,
    (width, height): (f32, f32),
    (hint, close): (&str, &str),
) -> bool {
    let edit = TextEdit::singleline(query)
        .id(id)
        .hint_text(format!("{}  {hint}", icon::FIND))
        .font(theme::body())
        .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(PALETTE_INSET, 0)))
        .vertical_align(Align::Center)
        .desired_width(f32::INFINITY);
    let response = ui.add_sized(vec2(width, height), edit);
    if !response.has_focus() {
        response.request_focus();
    }
    let rect = response.rect;
    keycap(
        ui.painter(),
        close,
        rect.right() - f32::from(PALETTE_INSET),
        rect.center().y,
    );
    let stroke = Stroke::new(1.0, theme::colors().hairline);
    ui.painter().hline(rect.x_range(), rect.bottom(), stroke);
    response.changed()
}

/// The bar's way into Find: a sunken box of the glyph, `label` and its `keys`, the glyph
/// alone in a square when `compact`.
pub fn search_button(ui: &mut Ui, label: &str, keys: &str, compact: bool) -> Response {
    let colors = theme::colors();
    let tint = colors.text_low;
    let width = match compact {
        true => FIND_COMPACT_W,
        false => {
            let painter = ui.painter();
            let word = painter.layout_no_wrap(label.to_owned(), theme::label(), tint);
            let keys_w = keycap_width(painter, keys);
            FIND_TEXT_X + word.size().x + FIND_KEYS_GAP + keys_w + FIND_KEYS_PAD
        }
    };
    let (rect, response) = ui.allocate_exact_size(vec2(width, FIND_H), Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    let painter = ui.painter();
    let stroke = hover_mix(ui, &response, colors.hairline, colors.line);
    painter.rect(
        rect,
        theme::R_SURFACE,
        colors.sunken,
        Stroke::new(1.0, stroke),
        egui::StrokeKind::Inside,
    );
    let glyph_x = if compact {
        rect.center().x
    } else {
        rect.left() + 15.0
    };
    let y = rect.center().y;
    painter.text(
        pos2(glyph_x, y),
        Align2::CENTER_CENTER,
        icon::FIND,
        theme::icon(13.0),
        tint,
    );
    if !compact {
        let at = pos2(rect.left() + FIND_TEXT_X, y);
        painter.text(at, Align2::LEFT_CENTER, label, theme::label(), tint);
        keycap(painter, keys, rect.right() - FIND_KEYS_PAD, y);
    }
    focus_ring(ui, &response, theme::R_SURFACE);
    response
}

/// Find's box: where its word starts, the room between it and the keys and after them,
/// points; a 30 square when compact.
const FIND_TEXT_X: f32 = 28.0;
const FIND_KEYS_GAP: f32 = 12.0;
const FIND_KEYS_PAD: f32 = 8.0;
const FIND_H: f32 = 30.0;
const FIND_COMPACT_W: f32 = 30.0;
