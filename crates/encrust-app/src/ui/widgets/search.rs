//! The two boxes a list is narrowed by: a window's find line and Find's palette field.

use egui::{Align, Id, Key, Modifiers, Stroke, TextEdit, Ui, vec2};

use super::field::text_box;
use super::keycap::keycap;
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
