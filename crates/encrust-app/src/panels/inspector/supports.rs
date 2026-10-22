use core_supports::Part;

use super::scope;
use crate::i18n::tr;
use crate::job::SupportJob;
use crate::panels::Window;
use crate::panels::support_fields::{held_up, saved_as, sizes};
use crate::state::Doc;
use crate::ui::{
    describe, hint, icon, primary_button, progress_bar, secondary_button, section, stats, theme,
    tone,
};

/// Growing supports under whatever hangs over nothing, and the parts of those already
/// standing, which a click in this tool picks.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    super::shape::profile_section(ui, window, true);
    section(ui, &tr!("supports-where"), None, |ui| {
        scope::editing_supports(window, |window| {
            if window.tools.supports.auto.is_some() {
                auto_held_up(ui, window);
            } else {
                let profile = &mut window.tools.supports.profile;
                let saved = saved_as(&window.machine.slicing.catalogue, profile)
                    .map(|entry| &entry.profile);
                held_up(ui, profile, saved);
            }
            if let [id] = *window.doc.scene.selection() {
                super::position::lift(ui, &mut window.doc.scene, id);
            }
        });
    });
    if window.tools.supports.auto.is_some() {
        section(ui, &tr!("supports-sizes"), None, |ui| {
            auto_sizes(ui, window)
        });
    }
    section(ui, &tr!("supports-picked"), None, |ui| editing(ui, window));
    section(ui, &tr!("supports-already-done"), None, |ui| {
        placed(ui, window)
    });
}

/// Where the automatic profile chosen puts supports, against the one it ships as. Each
/// support is then sized by what it holds; see `docs/decisions/0325`.
fn auto_held_up(ui: &mut egui::Ui, window: &mut Window) {
    let Some(auto) = window.tools.supports.auto.as_mut() else {
        return;
    };
    let shipped = window
        .machine
        .slicing
        .catalogue
        .autos()
        .find(|entry| entry.profile.name == auto.name)
        .map(|entry| entry.profile.placing_with(&window.tools.supports.profile));
    let mut placing = auto.placing_with(&window.tools.supports.profile);
    held_up(ui, &mut placing, shipped.as_ref());
    auto.take_placing(&placing);
}

/// Where the automatic profile chosen moves each support to a heavier shape, by the area of
/// the island it holds and by how much of the model stands over the overhang it holds.
fn auto_sizes(ui: &mut egui::Ui, window: &mut Window) {
    let catalogue = &window.machine.slicing.catalogue;
    let Some(auto) = window.tools.supports.auto.as_mut() else {
        return;
    };
    let shipped = catalogue
        .autos()
        .find(|entry| entry.profile.name == auto.name)
        .map(|entry| entry.profile.clone());
    let shapes = auto.shapes.each_ref().map(|id| {
        catalogue
            .support(id)
            .map_or_else(|_| id.clone(), |entry| entry.profile.name.clone())
    });
    sizes(ui, auto, shipped.as_ref(), &shapes);
}

/// How many supports stand on the plate, for the inspector's heading.
pub fn fact(window: &Window) -> Option<String> {
    let (points, _) = standing(window.doc);
    (points > 0).then(|| points.to_string())
}

/// The run that grows them, or its progress while it goes.
pub fn action(ui: &mut egui::Ui, window: &mut Window) {
    match window.tools.supports.job.as_ref() {
        Some(job) => running(ui, job),
        None => generate(ui, window),
    }
}

/// How many supports the plate carries, and how many of them have room to stand.
fn standing(doc: &Doc) -> (usize, usize) {
    doc.scene
        .targets()
        .fold((0, 0), |(points, columns), object| {
            (
                points + object.supports.point_count(),
                columns + object.supports.standing(),
            )
        })
}

/// What the click has picked: how much of it, which groups it belongs to, and the
/// three things that can be done to it without the mouse.
fn editing(ui: &mut egui::Ui, window: &mut Window) {
    if window.tools.supports.picked.is_empty() {
        hint(ui, &tr!("supports-pick-hint"));
        return;
    }
    describe(ui, &tr!("supports-picked-help"));

    let held = window.tools.supports.picked.clone();
    let supports = held
        .iter()
        .map(|picked| (picked.id, picked.frozen))
        .collect::<std::collections::BTreeSet<_>>();
    stats(
        ui,
        &[
            (tr!("supports-stat-parts"), held.len().to_string()),
            (tr!("supports-stat-supports"), supports.len().to_string()),
        ],
    );

    ui.add_space(4.0);
    let active = window.tools.supports.active;
    if primary_button(ui, icon::SUPPORTS, &tr!("supports-move-to-group"), true).clicked() {
        for (id, frozen) in &supports {
            if let Some(object) = window.doc.scene.get_mut(*id)
                && let Some(tree) = object.supports.frozen_mut(*frozen)
            {
                tree.set_group(active);
            }
        }
    }

    ui.add_space(4.0);
    if secondary_button(ui, icon::SUPPORTS, &tr!("supports-grow-these-again")).clicked() {
        // Highest first: thawing renumbers everything above what was taken out.
        for (id, frozen) in supports.iter().rev() {
            if let Some(object) = window.doc.scene.get_mut(*id) {
                let transform = object.transform;
                object.supports.thaw(*frozen, transform);
            }
        }
        window.tools.supports.picked.clear();
    }

    ui.add_space(4.0);
    if secondary_button(ui, icon::REMOVE, &tr!("supports-remove-these")).clicked() {
        // Highest first: a support that goes whole renumbers everything above it.
        for (id, frozen) in supports.iter().rev() {
            let parts: Vec<Part> = held
                .iter()
                .filter(|picked| picked.id == *id && picked.frozen == *frozen)
                .map(|picked| picked.part)
                .collect();
            if let Some(object) = window.doc.scene.get_mut(*id) {
                object.supports.remove_parts(*frozen, &parts);
            }
        }
        window.tools.supports.picked.clear();
    }
}

fn generate(ui: &mut egui::Ui, window: &mut Window) {
    let blocker = window.tools.supports.blocker(&window.doc.scene);
    let label = if standing(window.doc).0 > 0 {
        tr!("supports-grow-again")
    } else {
        tr!("supports-grow")
    };
    let what = tr!("supports-grow-tooltip", scope = window.doc.scene.scope());
    if primary_button(ui, icon::SUPPORTS, &label, blocker.is_none())
        .on_hover_text(what)
        .clicked()
    {
        let started = window.tools.supports.start(
            &window.doc.scene,
            window.machine.slicing.layer_height_mm(),
            &window.machine.slicing.catalogue,
        );
        window
            .machine
            .status
            .report(&tr!("supports-placing"), started);
    }
    if let Some(reason) = blocker {
        ui.vertical_centered(|ui| tone(ui, &reason, theme::colors().text_low));
    }
}

/// The progress bar and the cancel button of the run that is going.
fn running(ui: &mut egui::Ui, job: &SupportJob) {
    tone(ui, &job.label(), theme::colors().text_mid);
    progress_bar(ui, job.fraction());

    ui.add_enabled_ui(!job.is_cancelling(), |ui| {
        if secondary_button(ui, icon::CANCEL, &tr!("common-cancel")).clicked() {
            job.cancel();
        }
    });
}

/// How many supports are on the plate, and the one way to take them all off again.
fn placed(ui: &mut egui::Ui, window: &mut Window) {
    let (points, columns) = standing(window.doc);

    stats(
        ui,
        &[
            (tr!("supports-stat-supports"), points.to_string()),
            (tr!("supports-stat-standing"), columns.to_string()),
        ],
    );

    if points == 0 {
        return;
    }
    if points != columns {
        ui.add_space(4.0);
        hint(ui, &tr!("supports-no-room"));
    }

    // Side by side: everything, or only what stands in the group in hand.
    ui.add_space(4.0);
    let active = window.tools.supports.active;
    let name = window.tools.supports.groups[active as usize].name.clone();
    let in_group: usize = window
        .doc
        .scene
        .targets()
        .map(|object| object.supports.group_count(active))
        .sum();
    let (mut all, mut group) = (false, false);
    ui.columns(2, |columns| {
        all = secondary_button(&mut columns[0], icon::REMOVE, &tr!("supports-remove-all"))
            .on_hover_text(tr!("supports-remove-all-tooltip"))
            .clicked();
        group = columns[1]
            .add_enabled_ui(in_group > 0, |ui| {
                secondary_button(ui, icon::REMOVE, &tr!("supports-remove-group", name = name))
                    .on_hover_text(tr!("supports-remove-group-tooltip", count = in_group))
                    .clicked()
            })
            .inner;
    });
    for object in window.doc.scene.targets_mut() {
        if all {
            object.supports.clear();
        } else if group {
            object.supports.clear_group(active);
        }
    }
    // What was held may be gone, and what is left of the frozen is renumbered.
    if all || group {
        window.tools.supports.picked.clear();
    }
}
