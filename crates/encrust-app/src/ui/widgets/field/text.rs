use egui::{Align, Ui, vec2};

use super::{field_label, name_for_reader};
use crate::i18n::tr;
use crate::ui::theme;

/// The box an IP address is typed into, as tall as the buttons beside it and `width`
/// points wide.
pub fn address_field(ui: &mut Ui, text: &mut String, hint: &str, width: f32) -> egui::Response {
    let field = text_box(egui::TextEdit::singleline(text))
        .hint_text(hint)
        .font(theme::figures(13.0))
        .background_color(theme::colors().sunken);
    let response = ui.add_sized(vec2(width, theme::FIELD_H), field);
    name_for_reader(ui, response.id, &tr!("field-printer-address"));
    response
}

/// A line of text in the one shape every typed box has: as tall as a button, its text
/// centred in it.
pub fn text_box(edit: egui::TextEdit<'_>) -> egui::TextEdit<'_> {
    edit.min_size(vec2(0.0, theme::FIELD_H))
        .vertical_align(Align::Center)
        .margin(egui::Margin::symmetric(10, 0))
}

/// A box several lines of text are typed into, full width and `height` points tall, named
/// over it in the form's label look.
pub fn text_area(ui: &mut Ui, label: &str, text: &mut String, height: f32) -> egui::Response {
    field_label(ui, label, theme::colors().text_mid, "");
    ui.add_space(theme::NOTE_GAP);
    let edit = egui::TextEdit::multiline(text)
        .font(theme::label())
        .margin(egui::Margin::symmetric(10, 8))
        .background_color(theme::colors().sunken);
    let response = ui.add_sized(vec2(ui.available_width(), height), edit);
    name_for_reader(ui, response.id, label);
    response
}

/// Text shown to be read whole, not typed: set in the code face on `base` in a hairline box
/// as tall as the text, so it scrolls with what holds it.
pub fn text_block(ui: &mut Ui, text: &str) {
    let colors = theme::colors();
    egui::Frame::new()
        .fill(colors.base)
        .stroke(egui::Stroke::new(1.0, colors.hairline))
        .corner_radius(theme::R_CONTROL)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            crate::ui::paragraph(
                ui,
                egui::RichText::new(text)
                    .font(theme::code(11.0))
                    .color(colors.text_mid),
            );
        });
}

/// The name of a row whose box takes what is left, in a column of `ROW_NAME_W` so every
/// such box of a form starts on one line; a longer name is cut.
pub fn row_name(ui: &mut Ui, text: &str) {
    let cell = vec2(theme::ROW_NAME_W, theme::FIELD_H);
    let layout = egui::Layout::left_to_right(Align::Center);
    ui.allocate_ui_with_layout(cell, layout, |ui| {
        ui.set_width(theme::ROW_NAME_W);
        field_label(ui, text, theme::colors().text_mid, "");
    });
}

/// A named line of text: the name in its column, the field taking what is left.
pub fn text_row(ui: &mut Ui, label: &str, value: &mut String) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        row_name(ui, label);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let field = text_box(egui::TextEdit::singleline(value))
                .font(theme::label())
                .background_color(theme::colors().sunken);
            let response = ui.add_sized(vec2(ui.available_width(), theme::FIELD_H), field);
            name_for_reader(ui, response.id, label);
            changed = response.changed();
        });
    });
    changed
}

/// Why what was typed is refused: one line in `danger`, cut short and read whole on hover.
pub fn field_error(ui: &mut Ui, error: &str) {
    let line = egui::RichText::new(error)
        .font(theme::small())
        .color(theme::colors().danger);
    ui.add(egui::Label::new(line).truncate())
        .on_hover_text(error);
}
