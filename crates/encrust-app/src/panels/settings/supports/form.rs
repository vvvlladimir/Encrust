//! A support profile's form: its name, then its groups one under another from the tip down
//! to the plate, then how supports are placed.

use egui::Ui;
use printer_profiles::SupportProfile;

use crate::i18n::tr;
use crate::panels::Window;
use crate::panels::settings::jump::{self, Tab};
use crate::panels::settings::{form_card, name_row, report, switched_card};
use crate::panels::support_fields::Group;
use crate::settings::Draft;
use crate::state::Machine;
use crate::ui::describe;

/// The groups in the order the form lists them and its tabs name them.
const ORDER: [Group; 9] = [
    Group::Top,
    Group::Main,
    Group::Bottom,
    Group::SmallPillar,
    Group::Branching,
    Group::Grid,
    Group::Raft,
    Group::Bracing,
    Group::Automatic,
];

/// The tabs over the form: a group each, marked while its switch is off.
pub(super) fn tabs(profile: &mut SupportProfile) -> Vec<Tab> {
    ORDER
        .iter()
        .map(|group| Tab {
            name: group.label(),
            off: group.switch(profile).is_some_and(|on| !*on),
        })
        .collect()
}

/// The profile open in the form, kept until it is saved. A number is marked where it leaves
/// the saved one; see `docs/design/profiles.md`.
pub(super) fn form(ui: &mut Ui, window: &mut Window, tabs: egui::Id) {
    let Some(draft) = window.machine.settings.support.as_mut() else {
        describe(ui, &tr!("support-pick-profile"));
        return;
    };
    let saved = draft.as_saved().clone();
    let profile = &mut draft.values;

    form_card(ui, &tr!("table-profile"), |ui| {
        if let Some(name) = name_row(ui, "support", &profile.name) {
            profile.name = name;
        }
    });

    let saved = Some(&saved);
    for (index, group) in ORDER.into_iter().enumerate() {
        jump::section(ui, tabs, index, ui.cursor().top());
        let mut on = group.switch(profile).map(|on| *on);
        let shown = on != Some(false);
        switched_card(ui, &group.label(), on.as_mut(), |ui| match shown {
            true => group.fields(ui, profile, saved),
            false => group.off(ui),
        });
        take_switch(ui, group, profile, on);
    }
}

/// Writes a switch flipped in a card's title into the group it turns on.
fn take_switch(ui: &Ui, group: Group, profile: &mut SupportProfile, on: Option<bool>) {
    if let (Some(on), Some(field)) = (on, group.switch(profile))
        && *field != on
    {
        *field = on;
        ui.ctx().request_repaint();
    }
}

/// What Reset to defaults puts back in `draft`, and why.
pub(super) fn stock(draft: &Draft<SupportProfile>) -> (SupportProfile, String) {
    let shipped = draft.as_shipped();
    let help = match shipped {
        Some(_) => tr!("support-reset-shipped-help"),
        None => tr!("support-reset-stock-help"),
    };
    (defaults(&draft.values, shipped), help)
}

/// What `profile` resets to: the shipped profile, or a new one's stock numbers, under its
/// own name.
fn defaults(profile: &SupportProfile, shipped: Option<&SupportProfile>) -> SupportProfile {
    SupportProfile {
        name: profile.name.clone(),
        ..shipped.cloned().unwrap_or_default()
    }
}

/// Writes the profile open in the form when it holds anything unsaved. Answers whether it
/// is written, the status saying why where it is not.
pub(super) fn save(machine: &mut Machine) -> bool {
    let Some(draft) = machine.settings.support.as_mut() else {
        return false;
    };
    if !draft.is_dirty() {
        return true;
    }
    let outcome = machine
        .slicing
        .catalogue
        .save_support(&draft.id, &draft.values);
    let saved = report(&mut machine.status, &tr!("support-cannot-save"), outcome).is_some();
    if saved {
        draft.mark_saved();
    }
    saved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shipped_profile_resets_to_the_shipped_numbers_under_its_own_name() {
        let shipped = SupportProfile::shipped("heavy").expect("the heavy preset ships");
        let mut edited = shipped.clone();
        edited.name = "Mine".to_owned();
        edited.middle.diameter_mm += 0.5;
        let reset = defaults(&edited, Some(&shipped));
        assert_eq!(reset.name, "Mine");
        assert_eq!(reset.middle, shipped.middle);
    }

    #[test]
    fn a_profile_nobody_shipped_resets_to_a_new_profiles_numbers() {
        let mut own = SupportProfile::shipped("heavy").expect("the heavy preset ships");
        own.name = "Mine".to_owned();
        let reset = defaults(&own, None);
        assert_eq!(
            reset,
            SupportProfile {
                name: "Mine".to_owned(),
                ..SupportProfile::default()
            }
        );
    }

    #[test]
    fn every_group_has_one_place_in_the_form() {
        assert_eq!(ORDER.len(), 9, "the nine groups of a support profile");
        for group in &ORDER {
            let times = ORDER.iter().filter(|other| *other == group).count();
            assert_eq!(times, 1, "{group:?} stands once in the form");
        }
    }
}
