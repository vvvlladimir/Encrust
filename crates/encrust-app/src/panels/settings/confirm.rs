//! The question asked before a profile is thrown away. Everything the Settings screen
//! writes is written as it settles and there is no undo on it (ADR 0120), so a deletion
//! is asked about first; see ADR 0196.

use printer_profiles::Catalogue;

use crate::settings::{Deleting, other_printers};
use crate::ui::{card, describe, inline_button, secondary_button, theme};

/// What the dialog is asking, in the words the user needs to answer it.
struct Question {
    title: &'static str,
    /// What goes, named.
    what: String,
    /// What else it takes with it, when it takes anything.
    cost: Option<String>,
}

/// The dialog over the screen, if a deletion is waiting. Answers with what was confirmed.
pub(super) fn ask(
    ctx: &egui::Context,
    catalogue: &Catalogue,
    deleting: &Deleting,
) -> Option<Answer> {
    let question = question(catalogue, deleting);
    let mut open = true;
    let mut answer = None;
    egui::Window::new(question.title)
        .collapsible(false)
        .resizable(false)
        .frame(card().inner_margin(theme::PANEL_MARGIN))
        .open(&mut open)
        .show(ctx, |ui| {
            ui.set_width(380.0);
            ui.label(egui::RichText::new(&question.what).font(theme::label()));
            if let Some(cost) = &question.cost {
                describe(ui, cost);
            }
            describe(ui, "This cannot be undone.");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if inline_button(ui, "", "Delete", true, true).clicked() {
                    answer = Some(Answer::Confirmed);
                }
                if secondary_button(ui, "", "Cancel").clicked() {
                    answer = Some(Answer::Cancelled);
                }
            });
        });
    match open {
        true => answer,
        false => Some(Answer::Cancelled),
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
            match others.is_empty() {
                true => Question {
                    title: "Take this resin off",
                    what: format!("Delete {name}?"),
                    cost: Some(
                        "No other printer has it, so it goes with its exposures rather \
                         than into the pool."
                            .to_owned(),
                    ),
                },
                false => Question {
                    title: "Take this resin off",
                    what: format!("Take {name} off this printer?"),
                    cost: Some(format!(
                        "It stays in the pool, measured on {}.",
                        others.join(", ")
                    )),
                },
            }
        }
        Deleting::Resin(resin) => {
            let name = resin_name(catalogue, resin);
            let others = other_printers(catalogue, "", resin).unwrap_or_default();
            Question {
                title: "Delete this resin",
                what: format!("Delete {name} and the exposures measured for it?"),
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
