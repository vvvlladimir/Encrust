//! A support profile's form: its groups in rows of two, the shape from the tip down on the
//! left beside how supports are placed and joined.

use egui::Ui;
use printer_profiles::SupportProfile;

use super::{TWO_COLUMNS_W, reset_row};
use crate::i18n::tr;
use crate::panels::Window;
use crate::panels::settings::{
    card_cell, card_row, form_card, name_row, report, settled, switched_card,
};
use crate::panels::support_fields::Group;
use crate::state::Machine;
use crate::ui::describe;

/// The groups row by row, the shape on the left and the placing on the right, so a row's
/// titles stand level. Narrow, the left column comes first, then the right.
const ROWS: [[Option<Group>; 2]; 5] = [
    [Some(Group::Top), Some(Group::Automatic)],
    [Some(Group::Main), Some(Group::Branching)],
    [Some(Group::Bottom), Some(Group::Raft)],
    [Some(Group::SmallPillar), Some(Group::Grid)],
    [None, Some(Group::Bracing)],
];

/// The profile open in the form, written back as it is edited. A shipped profile's numbers
/// are marked where they leave the shipped ones; see `docs/design/profiles.md`.
pub(super) fn form(ui: &mut Ui, window: &mut Window) {
    let Some(draft) = window.machine.settings.support.as_mut() else {
        describe(ui, &tr!("support-pick-profile"));
        return;
    };
    let shipped = draft.as_shipped().cloned();
    let profile = &mut draft.values;

    form_card(ui, &tr!("table-profile"), |ui| {
        if let Some(name) = name_row(ui, "support", &profile.name) {
            profile.name = name;
        }
        let help = match shipped {
            Some(_) => tr!("support-reset-shipped-help"),
            None => tr!("support-reset-stock-help"),
        };
        reset_row(ui, profile, defaults(profile, shipped.as_ref()), &help);
    });

    let saved = shipped.as_ref();
    if ui.available_width() < TWO_COLUMNS_W {
        let columns = [0, 1].map(|column| ROWS.iter().filter_map(move |row| row[column]));
        for group in columns.into_iter().flatten() {
            let mut on = group.switch(profile).map(|on| *on);
            switched_card(ui, &group.label(), on.as_mut(), |ui| {
                group.show(ui, profile, saved)
            });
            take_switch(ui, group, profile, on);
        }
    } else {
        ui.spacing_mut().item_spacing.x = 12.0;
        for row in ROWS {
            let mut switches = row.map(|group| group.and_then(|g| g.switch(profile).map(|on| *on)));
            card_row(ui, 2, |ui, column| {
                let Some(group) = row[column] else {
                    return;
                };
                let on = switches[column].as_mut();
                card_cell(ui, &group.label(), on, |ui| group.show(ui, profile, saved));
            });
            for (group, on) in row.into_iter().zip(switches) {
                if let Some(group) = group {
                    take_switch(ui, group, profile, on);
                }
            }
        }
    }

    if settled(ui) {
        autosave(window.machine);
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

/// What `profile` resets to: the shipped profile, or a new one's stock numbers, under its
/// own name.
fn defaults(profile: &SupportProfile, shipped: Option<&SupportProfile>) -> SupportProfile {
    SupportProfile {
        name: profile.name.clone(),
        ..shipped.cloned().unwrap_or_default()
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
    if report(&mut machine.status, &tr!("support-cannot-save"), outcome).is_some() {
        draft.mark_saved();
    }
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
    fn every_group_has_one_place_in_the_rows() {
        let placed: Vec<_> = ROWS.iter().flatten().flatten().collect();
        assert_eq!(placed.len(), 9, "the nine groups of a support profile");
        for group in &placed {
            let times = placed.iter().filter(|other| *other == group).count();
            assert_eq!(times, 1, "{group:?} stands once in the rows");
        }
    }
}
