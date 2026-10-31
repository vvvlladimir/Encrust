//! The question asked when a support profile's form is left with edits not yet saved
//! (ADR 0389).

use crate::i18n::tr;
use crate::ui::{card, describe, inline_button, theme};

/// How the question was answered.
pub(super) enum Answer {
    Save,
    Discard,
    Cancel,
}

/// The dialog over the window, naming the profile `name`. Answers once a button is pressed
/// or the dialog is dismissed, which is Cancel.
pub(super) fn ask(ctx: &egui::Context, name: &str) -> Option<Answer> {
    let mut answer = None;
    let modal = egui::Modal::new(egui::Id::new("unsaved-profile"))
        .frame(card().inner_margin(theme::PANEL_MARGIN))
        .show(ctx, |ui| {
            ui.set_width(380.0);
            ui.label(
                egui::RichText::new(tr!("unsaved-title", name = name))
                    .font(theme::dialog_title())
                    .color(theme::colors().text_high),
            );
            ui.add_space(6.0);
            describe(ui, &tr!("unsaved-what"));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if inline_button(ui, "", &tr!("settings-save"), true, true).clicked() {
                    answer = Some(Answer::Save);
                }
                if inline_button(ui, "", &tr!("unsaved-discard"), false, true).clicked() {
                    answer = Some(Answer::Discard);
                }
                if inline_button(ui, "", &tr!("common-cancel"), false, true).clicked() {
                    answer = Some(Answer::Cancel);
                }
            });
        });
    match modal.should_close() {
        true => Some(Answer::Cancel),
        false => answer,
    }
}
