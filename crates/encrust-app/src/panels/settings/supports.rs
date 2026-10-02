//! The support profiles: every measurement of one support, in the groups the Supports tool
//! folds them into. A group on the plate picks one of these and tunes its own copy.

use egui::{Align, Layout, RichText, Ui};
use printer_profiles::Kind;

use crate::panels::Window;
use crate::panels::settings::{form_card, name_row, report, settled};
use crate::panels::support_fields::Group;
use crate::settings::{can_delete, new_support};
use crate::state::Machine;
use crate::supports::{Editing, Parameters};
use crate::ui::{describe, icon, icon_button, inline_button, list as rows, list_row, theme};

/// Below this a form has no room for two columns of cards.
const TWO_COLUMNS_W: f32 = 720.0;

/// Every support profile, and the ways to add one or take one away.
pub fn list(ui: &mut Ui, machine: &mut Machine) {
    let mut added = None;
    let mut picked = None;
    let mut deleted = None;
    let open = machine
        .settings
        .support
        .as_ref()
        .map(|draft| draft.id.clone());
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Profiles")
                .font(theme::section())
                .color(theme::colors().text_low),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if icon_button(ui, icon::ADD, "New support profile").clicked() {
                added = Some(None);
            }
            if let Some(open) = &open
                && icon_button(ui, icon::DUPLICATE, "Duplicate this profile").clicked()
            {
                added = Some(Some(open.clone()));
            }
        });
    });
    ui.add_space(4.0);

    let catalogue = &machine.slicing.catalogue;
    rows(ui, |ui| {
        for entry in catalogue.supports() {
            let remove = can_delete(catalogue, Kind::Support, &entry.id)
                .then_some((icon::REMOVE, "Delete this profile"));
            let chosen = open.as_deref() == Some(entry.id.as_str());
            let action = list_row(ui, &entry.profile.name, None, chosen, remove, None);
            if action.picked {
                picked = Some(entry.id.clone());
            }
            if action.removed {
                deleted = Some(entry.id.clone());
            }
        }
    });

    let catalogue = &mut machine.slicing.catalogue;
    if let Some(from) = added {
        let outcome = new_support(catalogue, from.as_deref());
        picked = report(&mut machine.status, "cannot add a support profile", outcome);
    }
    if let Some(id) = deleted {
        let outcome = catalogue.forget_user_copy(Kind::Support, &id);
        report(
            &mut machine.status,
            "cannot delete the support profile",
            outcome,
        );
        picked = catalogue.supports().next().map(|entry| entry.id.clone());
    }
    if let Some(id) = picked {
        machine
            .settings
            .pick_support(&machine.slicing.catalogue, &id);
    }
}

/// The profile open in the list, written back as it is edited.
pub fn form(ui: &mut Ui, window: &mut Window) {
    let Some(draft) = window.machine.settings.support.as_mut() else {
        describe(ui, "Pick a support profile.");
        return;
    };

    let mut drawing = false;
    form_card(ui, "Profile", |ui| {
        if let Some(name) = name_row(ui, "support", &draft.values.name) {
            draft.values.name = name;
        }
        ui.add_space(4.0);
        drawing = inline_button(ui, icon::PARAMETERS, "Set on a drawing", false, true).clicked();
    });

    let profile = &mut draft.values;
    if ui.available_width() < TWO_COLUMNS_W {
        for group in Group::ALL {
            form_card(ui, group.label(), |ui| group.show(ui, profile));
        }
    } else {
        let (left, right) = Group::ALL.split_at(Group::ALL.len().div_ceil(2));
        ui.spacing_mut().item_spacing.x = 12.0;
        ui.columns(2, |columns| {
            for (ui, groups) in columns.iter_mut().zip([left, right]) {
                for group in groups {
                    form_card(ui, group.label(), |ui| group.show(ui, profile));
                }
            }
        });
    }

    if drawing {
        window.tools.supports.parameters = Some(Parameters::new(Editing::Settings));
    }
    if settled(ui) {
        autosave(window.machine);
    }
}

fn autosave(machine: &mut Machine) {
    let Some(draft) = machine.settings.support.as_mut() else {
        return;
    };
    if !draft.is_dirty() {
        return;
    }
    let outcome = machine
        .slicing
        .catalogue
        .save_support(&draft.id, &draft.values);
    if report(
        &mut machine.status,
        "cannot save the support profile",
        outcome,
    )
    .is_some()
    {
        draft.mark_saved();
    }
}
