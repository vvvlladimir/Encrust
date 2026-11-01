//! An automatic profile's form: where its supports go, and the sizes, steps and slopes it
//! builds them to. See `docs/decisions/0325` and `0326`.

use egui::Ui;
use printer_profiles::{AUTO_SIZES, AutoProfile, Catalogue, SupportProfile};

use super::reset_row;
use crate::i18n::tr;
use crate::panels::Window;
use crate::panels::settings::{form_card, report, settled};
use crate::panels::support_fields::{held_up, sizes, steadying, steps};
use crate::state::Machine;
use crate::ui::{field_label, theme};

/// The cards of the form, one under another.
#[derive(Clone, Copy)]
enum Card {
    Placing,
    Sizes,
    Steps,
    Steadying,
}

const ROWS: [[Card; 2]; 2] = [[Card::Placing, Card::Sizes], [Card::Steps, Card::Steadying]];

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
        shipped: Option<&AutoProfile>,
        shapes: &[String; AUTO_SIZES],
    ) {
        match self {
            Self::Placing => placing(ui, auto, shipped),
            Self::Sizes => sizes(ui, auto, shipped, shapes),
            Self::Steps => steps(ui, auto, shipped, shapes),
            Self::Steadying => steadying(ui, auto, shipped),
        }
    }
}

/// The automatic profile open in the form, written back as it is edited.
pub(super) fn form(ui: &mut Ui, window: &mut Window) {
    let catalogue = &window.machine.slicing.catalogue;
    let Some(draft) = window.machine.settings.auto.as_mut() else {
        return;
    };
    let shipped = draft.as_shipped().cloned();
    let shapes = shape_names(catalogue, &draft.values);
    let auto = &mut draft.values;

    form_card(ui, &tr!("table-profile"), |ui| {
        field_label(ui, &auto.name, theme::colors().text_mid, "");
        if let Some(shipped) = &shipped {
            let stock = AutoProfile {
                name: auto.name.clone(),
                ..shipped.clone()
            };
            reset_row(ui, auto, stock, &tr!("support-reset-auto-help"));
        }
    });

    let saved = shipped.as_ref();
    for card in ROWS.into_iter().flatten() {
        form_card(ui, &card.title(), |ui| card.show(ui, auto, saved, &shapes));
    }

    if settled(ui) {
        autosave(window.machine);
    }
}

/// Where the profile puts supports, read through the fields a support profile has for it.
fn placing(ui: &mut Ui, auto: &mut AutoProfile, shipped: Option<&AutoProfile>) {
    let stock = SupportProfile::default();
    let saved = shipped.map(|shipped| shipped.placing_with(&stock));
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

fn autosave(machine: &mut Machine) {
    let Some(draft) = machine.settings.auto.as_mut() else {
        return;
    };
    if !draft.is_dirty() {
        return;
    }
    let outcome = machine
        .slicing
        .catalogue
        .save_auto(&draft.id, &draft.values);
    if report(&mut machine.status, &tr!("support-cannot-save"), outcome).is_some() {
        draft.mark_saved();
    }
}
