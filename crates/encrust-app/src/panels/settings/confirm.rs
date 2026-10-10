//! The question asked before a profile is thrown away. Everything the Settings screen
//! writes is written as it settles and there is no undo on it (ADR 0120), so a deletion
//! is asked about first; see ADR 0196.

use printer_profiles::Catalogue;

use crate::settings::{Deleting, is_thrown_away_with_the_printer, other_printers};
use crate::ui::{card, describe, inline_button, secondary_button, theme};

/// What the dialog is asking, in the words the user needs to answer it.
struct Question {
    title: &'static str,
    /// What goes, named.
    what: String,
    /// What else it takes with it, when it takes anything.
    cost: Option<String>,
    /// What the button that answers yes says, which is the verb of the question.
    verb: &'static str,
    /// Whether what goes is gone for good, rather than kept somewhere it comes back from.
    gone: bool,
}

/// The dialog over the screen, if a deletion is waiting. Answers with what was confirmed.
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
                egui::RichText::new(question.title)
                    .font(theme::dialog_title())
                    .color(theme::colors().text_high),
            );
            ui.add_space(6.0);
            ui.label(egui::RichText::new(&question.what).font(theme::label()));
            if let Some(cost) = &question.cost {
                describe(ui, cost);
            }
            if question.gone {
                describe(ui, "This cannot be undone.");
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if inline_button(ui, "", question.verb, true, true).clicked() {
                    answer = Some(Answer::Confirmed);
                }
                if secondary_button(ui, "", "Cancel").clicked() {
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
                title: "Remove this machine",
                what: format!("Remove {name} and everything you changed on it?"),
                verb: "Remove",
                gone: true,
                cost: catalogue
                    .is_shipped(printer_profiles::Kind::Printer, id)
                    .then(|| {
                        "The machine stays in the library, so it can be added again with \
                         the numbers this build ships."
                            .to_owned()
                    }),
            }
        }
        Deleting::ResinOff { printer, resin } => {
            let name = resin_name(catalogue, resin);
            let others = other_printers(catalogue, printer, resin).unwrap_or_default();
            let thrown_away =
                is_thrown_away_with_the_printer(catalogue, printer, resin).unwrap_or(false);
            match thrown_away {
                true => Question {
                    title: "Take this resin off",
                    what: format!("Delete {name}?"),
                    verb: "Delete",
                    gone: true,
                    cost: Some(
                        "Nothing was ever typed into it, so it is not kept in the pool.".to_owned(),
                    ),
                },
                false => Question {
                    title: "Take this resin off",
                    what: format!("Take {name} off this printer?"),
                    verb: "Take off",
                    gone: false,
                    cost: Some(match others.is_empty() {
                        true => "It stays in the pool, for this or any other printer to \
                                 take back."
                            .to_owned(),
                        false => {
                            format!("It stays in the pool, measured on {}.", others.join(", "))
                        }
                    }),
                },
            }
        }
        Deleting::Resin(resin) => {
            let name = resin_name(catalogue, resin);
            let others = other_printers(catalogue, "", resin).unwrap_or_default();
            Question {
                title: "Delete this resin",
                what: format!("Delete {name} and the exposures measured for it?"),
                verb: "Delete",
                gone: true,
                cost: (!others.is_empty())
                    .then(|| format!("It comes off {} as well.", others.join(", "))),
            }
        }
    }
}

fn resin_name(catalogue: &Catalogue, id: &str) -> String {
    catalogue
        .resin(id)
        .map_or_else(|_| id.to_owned(), |entry| entry.profile.name.clone())
}
