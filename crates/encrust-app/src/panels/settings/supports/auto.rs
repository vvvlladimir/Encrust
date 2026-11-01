//! An automatic profile's form: where its supports go, and the sizes, steps and slopes it
//! builds them to. See `docs/decisions/0325` and `0326`.

use egui::Ui;
use printer_profiles::{AUTO_SIZES, AutoProfile, Catalogue, SupportProfile};

use crate::i18n::tr;
use crate::panels::Window;
use crate::panels::settings::jump::{self, Tab};
use crate::panels::settings::{form_card, report};
use crate::panels::support_fields::{held_up, sizes, steadying, steps};
use crate::settings::Draft;
use crate::state::Machine;
use crate::ui::{row_name, title_line};

/// The cards of the form, one under another.
#[derive(Clone, Copy)]
enum Card {
    Placing,
    Sizes,
    Steps,
    Steadying,
}

const ORDER: [Card; 4] = [Card::Placing, Card::Sizes, Card::Steps, Card::Steadying];

/// The tabs over the form: a card each.
pub(super) fn tabs() -> Vec<Tab> {
    ORDER
        .map(|card| Tab {
            name: card.title(),
            off: false,
        })
        .into()
}

impl Card {
    fn title(self) -> String {
        match self {
            Self::Placing => tr!("supports-placing"),
            Self::Sizes => tr!("supports-sizes"),
            Self::Steps => tr!("support-group-steps"),
            Self::Steadying => tr!("support-group-steadying"),
        }
    }

    fn show(
        self,
        ui: &mut Ui,
        auto: &mut AutoProfile,
        saved: Option<&AutoProfile>,
        shapes: &[String; AUTO_SIZES],
    ) {
        match self {
            Self::Placing => placing(ui, auto, saved),
            Self::Sizes => sizes(ui, auto, saved, shapes),
            Self::Steps => steps(ui, auto, saved, shapes),
            Self::Steadying => steadying(ui, auto, saved),
        }
    }
}

/// The automatic profile open in the form, kept until it is saved. Its name is the
/// shipped one's, so it is shown rather than typed.
pub(super) fn form(ui: &mut Ui, window: &mut Window, tabs: egui::Id) {
    let catalogue = &window.machine.slicing.catalogue;
    let Some(draft) = window.machine.settings.auto.as_mut() else {
        return;
    };
    let saved = draft.as_saved().clone();
    let shapes = shape_names(catalogue, &draft.values);
    let auto = &mut draft.values;

    form_card(ui, &tr!("table-profile"), |ui| {
        ui.horizontal(|ui| {
            row_name(ui, &tr!("settings-name"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                title_line(ui, &auto.name);
            });
        });
    });

    let saved = Some(&saved);
    for (index, card) in ORDER.into_iter().enumerate() {
        jump::section(ui, tabs, index, ui.cursor().top());
        form_card(ui, &card.title(), |ui| card.show(ui, auto, saved, &shapes));
    }
}

/// What Reset to defaults puts back in `draft`: the shipped numbers under its own name, or
/// nothing for a profile this build does not ship.
pub(super) fn stock(draft: &Draft<AutoProfile>) -> Option<AutoProfile> {
    let shipped = draft.as_shipped()?;
    Some(AutoProfile {
        name: draft.values.name.clone(),
        ..shipped.clone()
    })
}

/// Where the profile puts supports, read through the fields a support profile has for it.
fn placing(ui: &mut Ui, auto: &mut AutoProfile, saved: Option<&AutoProfile>) {
    let stock = SupportProfile::default();
    let saved = saved.map(|saved| saved.placing_with(&stock));
    let mut placing = auto.placing_with(&stock);
    held_up(ui, &mut placing, saved.as_ref());
    auto.take_placing(&placing);
}

/// The names of the support profiles `auto` sizes into, lightest first, or their ids where
/// the catalogue has none.
fn shape_names(catalogue: &Catalogue, auto: &AutoProfile) -> [String; AUTO_SIZES] {
    auto.shapes.each_ref().map(|id| {
        catalogue
            .support(id)
            .map_or_else(|_| id.clone(), |entry| entry.profile.name.clone())
    })
}

/// Writes the profile open in the form when it holds anything unsaved. Answers whether it
/// is written, the status saying why where it is not.
pub(super) fn save(machine: &mut Machine) -> bool {
    let Some(draft) = machine.settings.auto.as_mut() else {
        return false;
    };
    if !draft.is_dirty() {
        return true;
    }
    let outcome = machine
        .slicing
        .catalogue
        .save_auto(&draft.id, &draft.values);
    let saved = report(&mut machine.status, &tr!("support-cannot-save"), outcome).is_some();
    if saved {
        draft.mark_saved();
    }
    saved
}
