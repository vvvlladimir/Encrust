use anyhow::Context as _;

use crate::panels::Window;
use crate::panels::support_fields::Group;
use crate::settings::keep_support;
use crate::supports::{Editing, Parameters};
use crate::ui::{
    Segment, Segmented, describe, icon, icon_button, inline_button, later, secondary_button,
    section, subheading, theme,
};

/// One part of a support, which is what the Part switch shows the fields of.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Part {
    #[default]
    Tip,
    Body,
    Base,
    Raft,
    Brace,
}

impl Part {
    const ALL: [Self; 5] = [Self::Tip, Self::Body, Self::Base, Self::Raft, Self::Brace];

    fn label(self) -> &'static str {
        match self {
            Self::Tip => "Tip",
            Self::Body => "Body",
            Self::Base => "Base",
            Self::Raft => "Raft",
            Self::Brace => "Brace",
        }
    }

    /// The groups of the profile's fields this part is drawn from, tip down to plate.
    fn groups(self) -> &'static [Group] {
        match self {
            Self::Tip => &[Group::Top],
            Self::Body => &[Group::Main, Group::SmallPillar, Group::Branching],
            Self::Base => &[Group::Bottom],
            Self::Raft => &[Group::Raft],
            Self::Brace => &[Group::Bracing],
        }
    }
}

/// The shape of the supports in the group in hand, part by part, and the saved profile it
/// is built to.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    section(ui, "Profile", None, |ui| profile_picker(ui, window));
    section(ui, "Part", None, |ui| {
        describe(ui, "The fields of the part picked, in the group in hand.");
        let part = part_switch(ui);
        for group in part.groups() {
            if part.groups().len() > 1 {
                subheading(ui, group.label());
            }
            group.show(ui, &mut window.tools.supports.profile);
        }
    });
    section(ui, "Presets", None, |ui| {
        // TODO(step-8): presets of a whole support shape, and saving the one in hand as
        // another.
        later(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for name in ["Detail", "Structure", "Anchor", "Save current"] {
                    inline_button(ui, "", name, false, true);
                }
            });
        });
    });
}

/// The switch between the parts, kept for the window rather than for the plate.
fn part_switch(ui: &mut egui::Ui) -> Part {
    let id = ui.id().with("support-part");
    let mut part = ui
        .data(|data| data.get_temp::<Part>(id))
        .unwrap_or_default();
    let segments = Part::ALL.map(|part| Segment::new(part, part.label()));
    Segmented::new(&segments)
        .width(ui.available_width())
        .show(ui, &mut part);
    ui.data_mut(|data| data.insert_temp(id, part));
    part
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
    let mut drawing = false;
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("support-profile")
            .width((ui.available_width() - 2.0 * (theme::ICON_SIZE + theme::ITEM_GAP)).max(80.0))
            .selected_text(shown)
            .show_ui(ui, |ui| {
                for entry in catalogue.supports() {
                    let chosen = matching.as_deref() == Some(entry.id.as_str());
                    if ui.selectable_label(chosen, &entry.profile.name).clicked() {
                        picked = Some(entry.profile.clone());
                    }
                }
            });
        if icon_button(ui, icon::PARAMETERS, "Set the measurements on a drawing").clicked() {
            drawing = true;
        }
        manage = icon_button(ui, icon::SETTINGS, "Edit support profiles").clicked();
    });
    if drawing {
        window.tools.supports.parameters = Some(Parameters::new(Editing::Tool));
    }

    if let Some(profile) = picked {
        window.tools.supports.profile = profile;
    }
    if matching.is_none() {
        if secondary_button(ui, icon::SAVE, "Save as a profile").clicked() {
            let edited = window.tools.supports.profile.clone();
            let kept = keep_support(&mut window.machine.slicing.catalogue, &edited)
                .context("cannot save the support profile");
            if window
                .machine
                .status
                .report("Saved the support profile", kept)
                .is_some()
            {
                window
                    .machine
                    .settings
                    .open_supports(&window.machine.slicing.catalogue, &edited);
            }
        }
    } else if manage {
        let profile = window.tools.supports.profile.clone();
        window
            .machine
            .settings
            .open_supports(&window.machine.slicing.catalogue, &profile);
    }
}
