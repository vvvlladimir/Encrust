use egui::{Frame, RichText, Ui};

use crate::ui::theme;

/// What a card says when it has nothing to work on: a large glyph over what is missing and
/// how to put it there, centred.
pub fn empty_state(ui: &mut Ui, glyph: &str, text: &str, how: &str) {
    let colors = theme::colors();
    ui.vertical_centered(|ui| {
        ui.add_space(EMPTY_PAD);
        ui.label(
            RichText::new(glyph)
                .font(theme::icon(30.0))
                .color(colors.text_low),
        );
        ui.add_space(theme::ITEM_GAP);
        ui.label(
            RichText::new(text)
                .font(theme::section())
                .color(colors.text_high),
        );
        ui.add_space(2.0);
        ui.label(
            RichText::new(how)
                .font(theme::small())
                .color(colors.text_mid),
        );
        ui.add_space(theme::ITEM_GAP + 4.0);
    });
}

/// Room above and below an empty state, points.
const EMPTY_PAD: f32 = 16.0;

/// A verdict that has to be seen before anything else in its block, on a wash of `tint`:
/// a glyph in `tint`, the verdict in plain ink, and a line saying what to do about it.
pub fn notice(ui: &mut Ui, glyph: &str, tint: egui::Color32, title: &str, why: &str) {
    let colors = theme::colors();
    Frame::new()
        .fill(wash_of(tint))
        .corner_radius(theme::R_CONTROL)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                ui.label(RichText::new(glyph).font(theme::icon(16.0)).color(tint));
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.label(
                        RichText::new(title)
                            .font(theme::label())
                            .color(colors.text_high),
                    );
                    ui.label(
                        RichText::new(why)
                            .font(theme::small())
                            .color(colors.text_mid),
                    );
                });
            });
        });
}

/// The palette's wash of a state colour, so a verdict reads in its state at a glance.
fn wash_of(tint: egui::Color32) -> egui::Color32 {
    let colors = theme::colors();
    [
        (colors.warn, colors.warn_wash),
        (colors.danger, colors.danger_wash),
        (colors.ok, colors.ok_wash),
        (colors.accent, colors.accent_wash),
    ]
    .into_iter()
    .find_map(|(state, wash)| (state == tint).then_some(wash))
    .unwrap_or(colors.raised)
}

/// A section heading, used by the inspector and by the header row of a floating card.
pub fn heading(ui: &mut Ui, title: &str, hint_text: Option<&str>) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(title)
                .font(theme::section())
                .color(theme::colors().text_mid),
        );
        if let Some(hint_text) = hint_text {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                hint(ui, hint_text);
            });
        }
    });
}

/// The title of a card that states figures rather than a tool, in `text_high`, with
/// `aside` quieter at its right: the plate card's name and resin.
pub fn card_title(ui: &mut Ui, title: &str, aside: Option<&str>) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(title)
                .font(theme::section())
                .color(theme::colors().text_high),
        );
        if let Some(aside) = aside {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                hint(ui, aside);
            });
        }
    });
}

/// A heading inside a block: the group of fields under it belongs together.
pub fn subheading(ui: &mut Ui, title: &str) {
    ui.add_space(8.0);
    ui.label(
        RichText::new(title)
            .font(theme::small())
            .color(theme::colors().text_low),
    );
    ui.add_space(2.0);
}

/// A `subheading` carrying the switch that turns its group on. Returns whether it was
/// flipped.
pub fn subheading_with_switch(ui: &mut Ui, title: &str, on: &mut bool) -> bool {
    ui.add_space(8.0);
    let flipped = ui
        .horizontal(|ui| {
            ui.label(
                RichText::new(title)
                    .font(theme::small())
                    .color(theme::colors().text_low),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                crate::ui::widgets::bare_switch(ui, on, title).changed()
            })
            .inner
        })
        .inner;
    ui.add_space(2.0);
    flipped
}

/// A quiet aside: a unit, a count, a reason something is greyed out.
pub fn hint(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .font(theme::small())
            .color(theme::colors().text_low),
    );
}

/// A figure worked out from the fields over it, set flush with their right edge.
pub fn field_note(ui: &mut Ui, text: &str) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.label(
            RichText::new(text)
                .font(theme::small())
                .color(theme::colors().text_low),
        );
    });
}

/// Lines of numbers under a picker: the build volume, the exposure the resin asks for.
pub fn meta(ui: &mut Ui, lines: &[String]) {
    for line in lines {
        ui.label(
            RichText::new(line)
                .font(theme::figures(11.0))
                .color(theme::colors().text_low),
        );
    }
}
