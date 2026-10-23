use printer_profiles::{Raft, SupportProfile};
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::scope;
use crate::i18n::tr;
use crate::panels::Window;
use crate::panels::support_diagram::{self, Part};
use crate::panels::support_fields::{Group, saved_as};
use crate::supports::own_raft_shown;
use crate::ui::{
    Segment, Segmented, hint, icon, kept, secondary_button, section, subheading,
    subheading_with_switch,
};

/// The groups of the profile's fields a part is drawn from, tip down to plate.
fn groups(part: Part) -> &'static [Group] {
    match part {
        Part::Tip => &[Group::Top],
        Part::Body => &[Group::Main, Group::Branching, Group::Grid],
        Part::Base => &[Group::Bottom],
        Part::Raft => &[Group::Raft],
        Part::Brace => &[Group::Bracing],
    }
}

/// The shape of the supports in the group in hand, part by part. The raft shown is the
/// picked model's or the plate's, lent to the profile while it is drawn so the profile's
/// own never moves; see `docs/decisions/0253`.
pub(super) fn sections(ui: &mut egui::Ui, window: &mut Window) {
    scope::editing_supports(window, |window| {
        section(ui, &tr!("shape-part"), None, |ui| {
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
    });
}

/// The parts of a support, one at a time, and the numbers each is built to.
/// `profiles_raft` is the raft the profile in hand was saved with, which the one being
/// shown can be set back to.
fn parts(ui: &mut egui::Ui, window: &mut Window, profiles_raft: Raft) {
    {
        let plate = own_raft_shown(&window.doc.scene).map(|_| window.tools.supports.raft);
        let profile = &mut window.tools.supports.profile;
        let saved =
            saved_as(&window.machine.slicing.catalogue, profile).map(|entry| &entry.profile);
        if kept_switch(
            ui,
            "support-kind",
            &[
                (false, tr!("shape-regular")),
                (true, tr!("support-group-small-pillar")),
            ],
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
                        subheading_with_switch(ui, &group.label(), on);
                    }
                    None => subheading(ui, &group.label()),
                }
            }
            group.show(ui, profile, saved);
        }
        if part == Part::Raft {
            raft_owner(ui, profile, profiles_raft, plate);
        }
    }
}

/// Whose raft the numbers above are, and the way to take the profile's instead. `plate`
/// is the plate's raft while a picked model keeps one of its own, to go back to.
fn raft_owner(
    ui: &mut egui::Ui,
    profile: &mut SupportProfile,
    profiles_raft: Raft,
    plate: Option<Raft>,
) {
    ui.add_space(4.0);
    hint(
        ui,
        &if plate.is_some() {
            tr!("shape-raft-own")
        } else {
            tr!("shape-raft-plate")
        },
    );
    if let Some(plate) = plate
        && secondary_button(ui, icon::RESET, &tr!("shape-use-plate-raft")).clicked()
    {
        profile.raft = plate;
    }
    let label = tr!("shape-use-profile-raft", name = profile.name.clone());
    ui.add_enabled_ui(profile.raft != profiles_raft, |ui| {
        if secondary_button(ui, icon::RESET, &label).clicked() {
            profile.raft = profiles_raft;
        }
    });
}

/// A switch across the panel whose choice is kept for the window rather than for the
/// plate, under `key`.
fn kept_switch<T>(ui: &mut egui::Ui, key: &str, choices: &[(T, String)]) -> T
where
    T: Copy + Default + PartialEq + Serialize + DeserializeOwned,
{
    let id = ui.id().with(key);
    let mut chosen = kept::get::<T>(ui.ctx(), id).unwrap_or_default();
    let segments: Vec<Segment<'_, T>> = choices
        .iter()
        .map(|(value, label)| Segment::new(*value, label.as_str()))
        .collect();
    Segmented::new(&segments)
        .width(ui.available_width())
        .show(ui, &mut chosen);
    kept::set(ui.ctx(), id, &chosen);
    chosen
}
