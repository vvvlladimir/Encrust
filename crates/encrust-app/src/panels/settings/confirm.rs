//! What a deletion costs, read on its `confirm_button` before the second press, and the one
//! question still asked in a modal: giving up the profiles that would not read (ADR 0196).

use printer_profiles::{Catalogue, Kind, Unreadable};

use crate::i18n::tr;
use crate::settings::{Deleting, is_thrown_away_with_the_printer, other_printers};
use crate::ui::{card, describe, inline_button, secondary_button, theme};

/// What the dialog is asking, in the words the user needs to answer it.
struct Question {
    title: String,
    /// What goes, named.
    what: String,
    /// What else it takes with it, when it takes anything.
    cost: Option<String>,
    /// What the button that answers yes says, which is the verb of the question.
    verb: String,
}

/// The dialog over the window. Answers with what was confirmed.
pub(super) fn ask(ctx: &egui::Context, catalogue: &Catalogue) -> Option<Answer> {
    let question = reset_question(catalogue);
    let mut answer = None;
    // A modal of its own, so it stands over the window it was asked from.
    let modal = egui::Modal::new(egui::Id::new("confirm-deletion"))
        .frame(card().inner_margin(theme::PANEL_MARGIN))
        .show(ctx, |ui| {
            ui.set_width(380.0);
            ui.label(
                egui::RichText::new(&question.title)
                    .font(theme::dialog_title())
                    .color(theme::colors().text_high),
            );
            ui.add_space(6.0);
            ui.label(egui::RichText::new(&question.what).font(theme::label()));
            if let Some(cost) = &question.cost {
                describe(ui, cost);
            }
            describe(ui, &tr!("confirm-cannot-undo"));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if inline_button(ui, "", &question.verb, true, true).clicked() {
                    answer = Some(Answer::Confirmed);
                }
                if secondary_button(ui, "", &tr!("common-cancel")).clicked() {
                    answer = Some(Answer::Cancelled);
                }
            });
        });
    match modal.should_close() {
        true => Some(Answer::Cancelled),
        false => answer,
    }
}

/// How the question was answered.
pub(super) enum Answer {
    Confirmed,
    Cancelled,
}

/// What a deletion costs beyond what goes, said in the tooltip of the button that arms it.
pub(in crate::panels::settings) fn cost(catalogue: &Catalogue, deleting: &Deleting) -> String {
    match deleting {
        Deleting::Printer(id) if catalogue.is_shipped(Kind::Printer, id) => {
            tr!("confirm-machine-stays")
        }
        Deleting::ResinOff { printer, resin } => {
            let others = other_printers(catalogue, printer, resin).unwrap_or_default();
            let thrown_away =
                is_thrown_away_with_the_printer(catalogue, printer, resin).unwrap_or(false);
            match (thrown_away, others.is_empty()) {
                (true, _) => tr!("confirm-untouched-resin"),
                (false, true) => tr!("confirm-stays-in-pool"),
                (false, false) => tr!("confirm-stays-measured", printers = others.join(", ")),
            }
        }
        Deleting::Resin(resin) => {
            let others = other_printers(catalogue, "", resin).unwrap_or_default();
            match others.is_empty() {
                true => tr!("confirm-cannot-undo"),
                false => tr!("confirm-comes-off", printers = others.join(", ")),
            }
        }
        Deleting::Printer(_) | Deleting::Profile(..) => tr!("confirm-cannot-undo"),
    }
}

/// Throwing away the files that would not read: the copies of shipped profiles give way to
/// them, and a file under an id of the user's own is deleted, as nothing ships in its place.
fn reset_question(catalogue: &Catalogue) -> Question {
    let (copies, own): (Vec<&Unreadable>, Vec<&Unreadable>) = catalogue
        .unreadable()
        .iter()
        .partition(|file| catalogue.resets_to_shipped(file));
    let names = |files: &[&Unreadable]| {
        let names: Vec<String> = files
            .iter()
            .map(|file| profile_name(catalogue, file.kind, &file.id))
            .collect();
        names.join(", ")
    };
    if copies.is_empty() {
        return Question {
            title: tr!("confirm-delete-unreadable"),
            what: tr!("confirm-delete-own-named", names = names(&own)),
            verb: tr!("confirm-delete"),
            cost: None,
        };
    }
    Question {
        title: tr!("confirm-reset-unreadable"),
        what: tr!("confirm-reset-named", names = names(&copies)),
        verb: tr!("confirm-reset"),
        cost: (!own.is_empty()).then(|| tr!("confirm-reset-own-gone", names = names(&own))),
    }
}

/// What a profile is called, or its id when it has gone or never read.
fn profile_name(catalogue: &Catalogue, kind: Kind, id: &str) -> String {
    let name = match kind {
        Kind::Printer => catalogue.printer(id).ok().map(|entry| &entry.profile.name),
        Kind::Resin => catalogue.resin(id).ok().map(|entry| &entry.profile.name),
        Kind::Support => catalogue.support(id).ok().map(|entry| &entry.profile.name),
        Kind::Auto => catalogue
            .autos()
            .find(|entry| entry.id == id)
            .map(|entry| &entry.profile.name),
        Kind::Hollow | Kind::Drain => catalogue
            .presets(kind)
            .find(|entry| entry.id == id)
            .map(|entry| &entry.profile.name),
    };
    name.map_or_else(|| id.to_owned(), Clone::clone)
}
