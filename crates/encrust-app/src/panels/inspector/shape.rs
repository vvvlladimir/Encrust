use anyhow::Context as _;

use crate::panels::Window;
use crate::panels::support_diagram::{self, Part};
use crate::panels::support_fields::{Group, saved_as};
use crate::settings::keep_support;
use crate::ui::{
    Segment, Segmented, icon, icon_button, secondary_button, section, subheading,
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
    section(ui, "Profile", None, |ui| profile_picker(ui, window));
    section(ui, "Part", None, |ui| {
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
fn profile_picker(ui: &mut egui::Ui, window: &mut Window) {
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
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("support-profile")
            .width((ui.available_width() - theme::ICON_SIZE - theme::ITEM_GAP).max(80.0))
            .selected_text(shown)
            .show_ui(ui, |ui| {
                for entry in catalogue.supports() {
                    let chosen = matching.as_deref() == Some(entry.id.as_str());
                    if ui.selectable_label(chosen, &entry.profile.name).clicked() {
                        picked = Some(entry.profile.clone());
                    }
                }
            });
        manage = icon_button(ui, icon::SETTINGS, "Edit support profiles").clicked();
    });

    if let Some(profile) = picked {
        window.tools.supports.profile = profile;
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
        window
            .machine
            .status
            .report("Saved the support profile", kept);
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
