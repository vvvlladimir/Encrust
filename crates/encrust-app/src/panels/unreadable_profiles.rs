//! The note that some of the user's profile files would not read, and the way to give them
//! up for the shipped ones or delete them. See `docs/decisions/0344`.

use std::path::PathBuf;

use egui::Ui;

use crate::i18n::tr;
use crate::panels::Window;
use crate::ui::{help_mark, icon, icon_button, inline_button, theme, tone};

/// Whether any of the user's profile files was left out because it would not read, and the
/// note about them was not put away this run; a file that breaks after is noted again.
pub fn any(ctx: &egui::Context, window: &Window) -> bool {
    let files = paths(window);
    !files.is_empty() && ctx.data(|data| data.get_temp::<Vec<PathBuf>>(put_away())) != Some(files)
}

fn paths(window: &Window) -> Vec<PathBuf> {
    let files = window.machine.slicing.catalogue.unreadable();
    files.iter().map(|file| file.path.clone()).collect()
}

fn put_away() -> egui::Id {
    egui::Id::new("unreadable-profiles-put-away")
}

/// How many files would not read, why behind the question mark, *Reset* where a shipped
/// profile stands in for one of them, else *Delete*, asked about first, and a way to put it by.
pub fn row(ui: &mut Ui, window: &mut Window) {
    let catalogue = &window.machine.slicing.catalogue;
    let files = catalogue.unreadable();
    if files.is_empty() {
        return;
    }
    let reasons: Vec<String> = files
        .iter()
        .map(|file| {
            tr!(
                "notice-unreadable-file",
                path = file.path.display().to_string(),
                reason = reason(&*file.error)
            )
        })
        .collect();
    let action = if files.iter().any(|file| catalogue.resets_to_shipped(file)) {
        (icon::RESET, tr!("notice-reset-profiles"))
    } else {
        (icon::REMOVE, tr!("notice-delete-profiles"))
    };
    let label = tr!("notice-unreadable", count = files.len());
    ui.horizontal(|ui| {
        tone(ui, &label, theme::colors().warn);
        help_mark(ui, &reasons);
        if inline_button(ui, action.0, &action.1, true, true).clicked() {
            window.machine.settings.reset_unreadable = true;
        }
        if icon_button(ui, icon::CANCEL, &tr!("notice-put-away")).clicked() {
            let files = paths(window);
            ui.data_mut(|data| data.insert_temp(put_away(), files));
        }
    });
}

/// The error and every cause under it, outermost first.
fn reason(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut cause = error.source();
    while let Some(next) = cause {
        text.push_str(": ");
        text.push_str(&next.to_string());
        cause = next.source();
    }
    text
}
