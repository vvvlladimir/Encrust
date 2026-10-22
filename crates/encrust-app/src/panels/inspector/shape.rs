use anyhow::Context as _;
use printer_profiles::{Raft, SupportProfile};

use super::scope;
use crate::i18n::tr;
use crate::panels::Window;
use crate::panels::support_diagram::{self, Part};
use crate::panels::support_fields::{Group, saved_as};
use crate::settings::keep_support;
use crate::supports::own_raft_shown;
use crate::ui::{
    Segment, Segmented, describe, hairline, hint, icon, icon_button, secondary_button, section,
    subheading, subheading_with_switch, theme,
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
    sizing_switch(ui, window);
    scope::editing_supports(window, |window| shape(ui, window));
}

/// The profile the picked models' supports are built to, to be chosen, saved and opened
/// for editing from any tool that places supports, under the switch between sizing them
/// automatically and by hand. `sizes_run` lists the automatic profiles themselves while
/// sizing, for the tool whose run they size.
pub fn profile_section(ui: &mut egui::Ui, window: &mut Window, sizes_run: bool) {
    sizing_switch(ui, window);
    scope::editing_supports(window, |window| {
        section(ui, &tr!("table-profile"), None, |ui| {
            describe(ui, &tr!("shape-profile-help"));
            profile_picker(ui, window, false, sizes_run);
        });
    });
}

/// Whether supports are sized automatically by what they hold or built to a profile
/// chosen by hand, across the top of every tool that places or shapes them: which
/// profiles the lists offer, and what a run places. See `docs/decisions/0325`.
fn sizing_switch(ui: &mut egui::Ui, window: &mut Window) {
    let mut automatic = window.tools.supports.auto.is_some();
    egui::Frame::new()
        .inner_margin(theme::PANEL_MARGIN)
        .show(ui, |ui| {
            let segments = [
                Segment::new(true, tr!("supports-sizing-auto")),
                Segment::new(false, tr!("supports-sizing-manual")),
            ];
            Segmented::new(&segments)
                .width(ui.available_width())
                .show(ui, &mut automatic);
        });
    hairline(ui);
    window
        .tools
        .supports
        .set_automatic(automatic, &window.machine.slicing.catalogue);
}

fn shape(ui: &mut egui::Ui, window: &mut Window) {
    section(ui, &tr!("table-profile"), None, |ui| {
        describe(ui, &tr!("shape-profile-help"));
        profile_picker(ui, window, true, false);
    });
    section(ui, &tr!("shape-part"), None, |ui| {
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
    T: Copy + Default + PartialEq + Send + Sync + 'static,
{
    let id = ui.id().with(key);
    let mut chosen = ui.data(|data| data.get_temp::<T>(id)).unwrap_or_default();
    let segments: Vec<Segment<'_, T>> = choices
        .iter()
        .map(|(value, label)| Segment::new(*value, label.as_str()))
        .collect();
    Segmented::new(&segments)
        .width(ui.available_width())
        .show(ui, &mut chosen);
    ui.data_mut(|data| data.insert_temp(id, chosen));
    chosen
}

/// Which saved profile the group in hand is built to, and the way to the screen where
/// profiles are made, edited and thrown away. The list offers the shapes of the sizing
/// switched to, or the automatic profiles themselves where `sizes_run` while sizing.
fn profile_picker(ui: &mut egui::Ui, window: &mut Window, with_manage: bool, sizes_run: bool) {
    let catalogue = &window.machine.slicing.catalogue;
    let automatic = window.tools.supports.auto.is_some();
    let auto = window.tools.supports.auto.as_ref().filter(|_| sizes_run);
    let current = &window.tools.supports.profile;
    let matching = catalogue
        .supports()
        .find(|entry| auto.is_none() && entry.profile == *current)
        .map(|entry| entry.id.clone());
    let auto_matching = auto.and_then(|auto| {
        catalogue
            .autos()
            .find(|entry| entry.profile == *auto)
            .map(|entry| entry.id.clone())
    });
    let shown = match (auto, &matching, &auto_matching) {
        (Some(auto), _, Some(_)) => auto.name.clone(),
        (Some(auto), _, None) => tr!("shape-edited", name = auto.name.clone()),
        (None, Some(_), _) => current.name.clone(),
        (None, None, _) => tr!("shape-edited", name = current.name.clone()),
    };

    let mut picked = None;
    let mut picked_auto = None;
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
                if auto.is_some() {
                    for entry in catalogue.autos() {
                        let chosen = auto_matching.as_deref() == Some(entry.id.as_str());
                        if ui.selectable_label(chosen, &entry.profile.name).clicked() {
                            picked_auto = Some(entry.profile.clone());
                        }
                    }
                    return;
                }
                let offered = catalogue
                    .supports()
                    .filter(|entry| catalogue.is_auto_shape(&entry.id) == automatic);
                for entry in offered {
                    let chosen = matching.as_deref() == Some(entry.id.as_str());
                    if ui.selectable_label(chosen, &entry.profile.name).clicked() {
                        picked = Some(entry.profile.clone());
                    }
                }
            });
        if with_manage {
            manage = icon_button(ui, icon::SETTINGS, &tr!("shape-edit-profiles")).clicked();
        }
    });

    let in_auto = auto.is_some();
    if picked_auto.is_some() {
        window.tools.supports.auto = picked_auto;
    }
    // Not the numbers in hand: another profile is another kind of support, and what
    // stands stays as it was built. See `docs/decisions/0247`.
    if picked.is_some() {
        window.tools.supports.chosen = picked;
    }
    // Both always there, and dead while there is nothing to save: a button that comes and
    // goes moves everything under it. An automatic profile ships and is not saved.
    let edited = matching.is_none() && !in_auto;
    let source = saved_as(
        &window.machine.slicing.catalogue,
        &window.tools.supports.profile,
    )
    .map(|entry| entry.id.clone());
    let (mut save, mut save_as) = (false, false);
    // An automatic profile ships and is not saved, so it has no buttons to grey out.
    if !in_auto {
        ui.columns(2, |columns| {
            save = columns[0]
                .add_enabled_ui(edited && source.is_some(), |ui| {
                    secondary_button(ui, icon::SAVE, &tr!("shape-save"))
                        .on_hover_text(tr!("shape-save-tooltip"))
                        .clicked()
                })
                .inner;
            save_as = columns[1]
                .add_enabled_ui(edited, |ui| {
                    secondary_button(ui, icon::SAVE, &tr!("shape-save-as-new"))
                        .on_hover_text(tr!("shape-save-as-new-tooltip"))
                        .clicked()
                })
                .inner;
        });
    }

    let profile = window.tools.supports.profile.clone();
    if let Some(id) = source.filter(|_| save) {
        let kept = window
            .machine
            .slicing
            .catalogue
            .save_support(&id, &profile)
            .context(tr!("support-cannot-save"));
        if window
            .machine
            .status
            .report(&tr!("shape-saved"), kept)
            .is_some()
        {
            window.tools.supports.saved = Some(profile.clone());
        }
    }
    if save_as {
        let kept = keep_support(&mut window.machine.slicing.catalogue, &profile)
            .context(tr!("support-cannot-save"));
        if window
            .machine
            .status
            .report(&tr!("shape-saved"), kept)
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
