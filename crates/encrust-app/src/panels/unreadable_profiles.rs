//! The note that some of the user's profile files would not read, and the way to give them
//! up for the shipped ones. See `docs/decisions/0344`.

use egui::Ui;

use crate::i18n::tr;
use crate::panels::Window;
use crate::settings::Deleting;
use crate::ui::{help_mark, icon, inline_button, theme, tone};

/// Whether any of the user's profile files was left out because it would not read.
pub fn any(window: &Window) -> bool {
    !window.machine.slicing.catalogue.unreadable().is_empty()
}

/// How many files would not read, why behind the question mark, and *Reset* when a shipped
/// profile stands in for one of them; the reset is asked about first.
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
    let resettable = files.iter().any(|file| catalogue.resets_to_shipped(file));
    let label = tr!("notice-unreadable", count = files.len());
    ui.horizontal(|ui| {
        tone(ui, &label, theme::colors().warn);
        help_mark(ui, &reasons);
        if resettable
            && inline_button(ui, icon::RESET, &tr!("notice-reset-profiles"), true, true).clicked()
        {
            window.machine.settings.confirm = Some(Deleting::Unreadable);
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
