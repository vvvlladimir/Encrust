use anyhow::Context as _;
use printer_profiles::{Raft, SupportProfile};

use super::scope;
use crate::panels::Window;
use crate::panels::support_diagram::{self, Part};
use crate::panels::support_fields::{Group, saved_as};
use crate::settings::keep_support;
use crate::ui::{
    Segment, Segmented, hint, icon, icon_button, secondary_button, section, subheading,
    subheading_with_switch, theme,
};

/// The groups of the profile's fields a part is drawn from, tip down to plate.
fn groups(part: Part) -> &'static [Group] {
    match part {
        Part::Tip => &[Group::Top],
        Part::Body => &[Group::Main, Group::Branching],
        Part::Base => &[Group::Bottom],
        Part::Raft => &[Group::Raft],
        Part::Brace => &[Group::Bracing],
    }
}

/// The shape of the supports in the group in hand, part by part, and the saved profile it
/// is built to.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    scope::editing_supports(window, |window| shape(ui, window));
}

/// The profile the picked models' supports are built to, to be chosen, saved and opened
/// for editing from any tool that places supports.
pub fn profile_section(ui: &mut egui::Ui, window: &mut Window) {
    scope::editing_supports(window, |window| {
        section(ui, "Profile", None, |ui| profile_picker(ui, window, false));
    });
}

fn shape(ui: &mut egui::Ui, window: &mut Window) {
    section(ui, "Profile", None, |ui| profile_picker(ui, window, true));
    section(ui, "Part", None, |ui| {
        // The picked model's raft, or the plate's, lent to the profile while it is drawn so
        // the profile's own never moves. See `docs/decisions/0253`.
        let raft = window.tools.supports.raft_shown(&window.doc.scene);
        let profiles_own = std::mem::replace(&mut window.tools.supports.profile.raft, raft);
        parts(ui, window, profiles_own);
        let edited = std::mem::replace(&mut window.tools.supports.profile.raft, profiles_own);
        if edited != raft {
            window
                .tools
                .supports
                .keep_raft(edited, &mut window.doc.scene);
        }
    });
}

/// The parts of a support, one at a time, and the numbers each is built to.
/// `profiles_raft` is the raft the profile in hand was saved with, which the one being
/// shown can be set back to.
fn parts(ui: &mut egui::Ui, window: &mut Window, profiles_raft: Raft) {
    {
        let picked = !window.doc.scene.selection().is_empty();
        let profile = &mut window.tools.supports.profile;
        let saved =
            saved_as(&window.machine.slicing.catalogue, profile).map(|entry| &entry.profile);
        if kept_switch(
            ui,
            "support-kind",
            &[(false, "Regular"), (true, "Small pillar")],
        ) {
            support_diagram::small_pillar(ui, profile);
            ui.add_space(6.0);
            Group::SmallPillar.show(ui, profile, saved);
            return;
        }
        let parts = Part::ALL.map(|part| (part, part.label()));
        let part = kept_switch(ui, "support-part", &parts);
        support_diagram::regular(ui, profile, part);
        ui.add_space(6.0);
        for group in groups(part) {
            if groups(part).len() > 1 {
                match group.switch(profile) {
                    Some(on) => {
                        subheading_with_switch(ui, group.label(), on);
                    }
                    None => subheading(ui, group.label()),
                }
            }
            group.show(ui, profile, saved);
        }
        if part == Part::Raft {
            raft_owner(ui, profile, profiles_raft, picked);
        }
    }
}

/// Whose raft the numbers above are, and the way to take the profile's instead.
fn raft_owner(ui: &mut egui::Ui, profile: &mut SupportProfile, profiles_raft: Raft, picked: bool) {
    ui.add_space(4.0);
    hint(
        ui,
        if picked {
            "The raft is this model's, whichever profile its supports are built to."
        } else {
            "The raft is the plate's: every model without its own stands on one like it."
        },
    );
    let label = format!("Use the raft of {}", profile.name);
    ui.add_enabled_ui(profile.raft != profiles_raft, |ui| {
        if secondary_button(ui, icon::RESET, &label).clicked() {
            profile.raft = profiles_raft;
        }
    });
}

/// A switch across the panel whose choice is kept for the window rather than for the
/// plate, under `key`.
fn kept_switch<T>(ui: &mut egui::Ui, key: &str, choices: &[(T, &str)]) -> T
where
    T: Copy + Default + PartialEq + Send + Sync + 'static,
{
    let id = ui.id().with(key);
    let mut chosen = ui.data(|data| data.get_temp::<T>(id)).unwrap_or_default();
    let segments: Vec<Segment<'_, T>> = choices
        .iter()
        .map(|(value, label)| Segment::new(*value, label))
        .collect();
    Segmented::new(&segments)
        .width(ui.available_width())
        .show(ui, &mut chosen);
    ui.data_mut(|data| data.insert_temp(id, chosen));
    chosen
}

/// Which saved profile the group in hand is built to, and the way to the screen where
/// profiles are made, edited and thrown away.
fn profile_picker(ui: &mut egui::Ui, window: &mut Window, with_manage: bool) {
    let catalogue = &window.machine.slicing.catalogue;
    let current = &window.tools.supports.profile;
    let matching = catalogue
        .supports()
        .find(|entry| entry.profile == *current)
        .map(|entry| entry.id.clone());
    let shown = match &matching {
        Some(_) => current.name.clone(),
        None => format!("{} (edited)", current.name),
    };

    let mut picked = None;
    let mut manage = false;
    // The way to the profiles' own screen is on the Shape tool, where they are edited.
    let beside = if with_manage {
        theme::ICON_SIZE + theme::ITEM_GAP
    } else {
        0.0
    };
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("support-profile")
            .width((ui.available_width() - beside).max(80.0))
            .selected_text(shown)
            .show_ui(ui, |ui| {
                for entry in catalogue.supports() {
                    let chosen = matching.as_deref() == Some(entry.id.as_str());
                    if ui.selectable_label(chosen, &entry.profile.name).clicked() {
                        picked = Some(entry.profile.clone());
                    }
                }
            });
        if with_manage {
            manage = icon_button(ui, icon::SETTINGS, "Edit support profiles").clicked();
        }
    });

    // Not the numbers in hand: another profile is another kind of support, and what
    // stands stays as it was built. See `docs/decisions/0247`.
    if picked.is_some() {
        window.tools.supports.chosen = picked;
    }
    // Both always there, and dead while there is nothing to save: a button that comes and
    // goes moves everything under it.
    let edited = matching.is_none();
    let source = saved_as(
        &window.machine.slicing.catalogue,
        &window.tools.supports.profile,
    )
    .map(|entry| entry.id.clone());
    let (mut save, mut save_as) = (false, false);
    ui.columns(2, |columns| {
        save = columns[0]
            .add_enabled_ui(edited && source.is_some(), |ui| {
                secondary_button(ui, icon::SAVE, "Save")
                    .on_hover_text("Save over the profile these numbers were taken from")
                    .clicked()
            })
            .inner;
        save_as = columns[1]
            .add_enabled_ui(edited, |ui| {
                secondary_button(ui, icon::SAVE, "Save as new")
                    .on_hover_text("Keep these numbers as a profile of their own")
                    .clicked()
            })
            .inner;
    });

    let profile = window.tools.supports.profile.clone();
    if let Some(id) = source.filter(|_| save) {
        let kept = window
            .machine
            .slicing
            .catalogue
            .save_support(&id, &profile)
            .context("cannot save the support profile");
        if window
            .machine
            .status
            .report("Saved the support profile", kept)
            .is_some()
        {
            window.tools.supports.saved = Some(profile.clone());
        }
    }
    if save_as {
        let kept = keep_support(&mut window.machine.slicing.catalogue, &profile)
            .context("cannot save the support profile");
        if window
            .machine
            .status
            .report("Saved the support profile", kept)
            .is_some()
        {
            manage = true;
        }
    }
    if manage {
        window
            .machine
            .settings
            .open_supports(&window.machine.slicing.catalogue, &profile);
    }
}
