//! The question asked before a profile is thrown away. Undo reaches a form's numbers and
//! never a deleted file (ADR 0268), so a deletion is asked about first; see ADR 0196.

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
    /// Whether what goes is gone for good, rather than kept somewhere it comes back from.
    gone: bool,
}

/// The dialog over the window, if a deletion is waiting. Answers with what was confirmed.
pub(super) fn ask(
    ctx: &egui::Context,
    catalogue: &Catalogue,
    deleting: &Deleting,
) -> Option<Answer> {
    let question = question(catalogue, deleting);
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
            if question.gone {
                describe(ui, &tr!("confirm-cannot-undo"));
            }
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

/// What is about to go, and what goes with it.
fn question(catalogue: &Catalogue, deleting: &Deleting) -> Question {
    match deleting {
        Deleting::Printer(id) => {
            let name = catalogue.printer(id).map_or_else(
                |_| id.clone(),
                |entry| format!("{} {}", entry.profile.manufacturer, entry.profile.name),
            );
            Question {
                title: tr!("machines-remove"),
                what: tr!("confirm-remove-machine", name = name),
                verb: tr!("confirm-remove"),
                gone: true,
                cost: catalogue
                    .is_shipped(Kind::Printer, id)
                    .then(|| tr!("confirm-machine-stays")),
            }
        }
        Deleting::ResinOff { printer, resin } => {
            let name = profile_name(catalogue, Kind::Resin, resin);
            let others = other_printers(catalogue, printer, resin).unwrap_or_default();
            let thrown_away =
                is_thrown_away_with_the_printer(catalogue, printer, resin).unwrap_or(false);
            match thrown_away {
                true => Question {
                    title: tr!("confirm-take-resin-off"),
                    what: tr!("confirm-delete-named", name = name),
                    verb: tr!("profiles-delete"),
                    gone: true,
                    cost: Some(tr!("confirm-untouched-resin")),
                },
                false => Question {
                    title: tr!("confirm-take-resin-off"),
                    what: tr!("confirm-take-off-named", name = name),
                    verb: tr!("confirm-take-off"),
                    gone: false,
                    cost: Some(match others.is_empty() {
                        true => tr!("confirm-stays-in-pool"),
                        false => tr!("confirm-stays-measured", printers = others.join(", ")),
                    }),
                },
            }
        }
        Deleting::Resin(resin) => {
            let name = profile_name(catalogue, Kind::Resin, resin);
            let others = other_printers(catalogue, "", resin).unwrap_or_default();
            Question {
                title: tr!("confirm-delete-resin"),
                what: tr!("confirm-delete-resin-named", name = name),
                verb: tr!("profiles-delete"),
                gone: true,
                cost: (!others.is_empty())
                    .then(|| tr!("confirm-comes-off", printers = others.join(", "))),
            }
        }
        Deleting::Profile(kind, id) => Question {
            title: tr!("confirm-delete-profile"),
            what: tr!(
                "confirm-delete-named",
                name = profile_name(catalogue, *kind, id)
            ),
            verb: tr!("profiles-delete"),
            gone: true,
            cost: None,
        },
        Deleting::Unreadable => reset_question(catalogue),
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
            title: tr!("confirm-reset-unreadable"),
            what: tr!("confirm-delete-own-named", names = names(&own)),
            verb: tr!("confirm-delete"),
            gone: true,
            cost: None,
        };
    }
    Question {
        title: tr!("confirm-reset-unreadable"),
        what: tr!("confirm-reset-named", names = names(&copies)),
        verb: tr!("confirm-reset"),
        gone: true,
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
