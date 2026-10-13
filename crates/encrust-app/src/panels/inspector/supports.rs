use core_analysis::RiskKind;
use core_supports::Part;

use super::scope;
use crate::job::SupportJob;
use crate::panels::Window;
use crate::panels::support_fields::{Group, saved_as};
use crate::state::Doc;
use crate::status::Status;
use crate::ui::{
    describe, hint, icon, primary_button, progress_bar, secondary_button, section, stats, theme,
    tone,
};
use crate::workspace::{Mode, Tool};

/// Growing supports under whatever hangs over nothing, and the parts of those already
/// standing, which a click in this tool picks.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    super::shape::profile_section(ui, window);
    section(ui, "Islands", None, |ui| islands(ui, window));
    section(ui, "Where", None, |ui| {
        scope::editing_supports(window, |window| {
            let profile = &mut window.tools.supports.profile;
            let saved =
                saved_as(&window.machine.slicing.catalogue, profile).map(|entry| &entry.profile);
            Group::Automatic.show(ui, profile, saved);
            lift(ui, window);
        });
    });
    section(ui, "Picked", None, |ui| editing(ui, window));
    section(ui, "Already done", None, |ui| placed(ui, window));
}

/// Layer parts with nothing under them, read from the same cut stack the Check tool reads,
/// supports and all: an island counted here is one the supports do not hold yet.
fn islands(ui: &mut egui::Ui, window: &mut Window) {
    describe(
        ui,
        "Layer parts with nothing under them. They cure loose in the vat.",
    );
    let colors = theme::colors();
    let found = window.measured().map(|measured| {
        let islands: Vec<usize> = measured
            .risks()
            .iter()
            .filter(|risk| {
                matches!(
                    risk.kind,
                    RiskKind::Island { .. } | RiskKind::Floating { .. }
                )
            })
            .map(|risk| risk.layer)
            .collect();
        (islands.len(), islands.first().copied())
    });
    match found {
        Some((0, _)) => tone(ui, "Every island is held.", colors.ok),
        Some((loose, first)) => {
            let from = first.map_or(String::new(), |layer| {
                format!(", the first at layer {}", layer + 1)
            });
            tone(ui, &format!("{loose} cure loose{from}."), colors.danger);
            if secondary_button(ui, icon::CHECK, "Show them in Check").clicked() {
                crate::shortcuts::pick(window, Tool::Check);
            }
        }
        None if window.machine.preview.is_measuring() || window.machine.preview.is_building() => {
            ui.horizontal(|ui| {
                ui.spinner();
                hint(ui, "Reading every layer of the stack...");
            });
        }
        None => {
            hint(
                ui,
                "Not scanned yet. Scanning cuts the layers and shows them beside the model.",
            );
            if secondary_button(ui, icon::CHECK, "Scan").clicked() {
                *window.mode = Mode::Preview;
                window.view.options.mask_only = false;
            }
        }
    }
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

/// Standing every part clear of the plate, by the lift the profile carries.
fn lift(ui: &mut egui::Ui, window: &mut Window) {
    let label = format!(
        "Lift models {:.1} mm",
        window.tools.supports.profile.z_lift_mm
    );
    if secondary_button(ui, icon::SUPPORTS, &label).clicked() {
        let moved = window.tools.supports.lift(&mut window.doc.scene);
        window.machine.status = Status::Info(match moved {
            0 => "Nothing on the plate to lift".to_owned(),
            1 => "Lifted 1 model off the plate".to_owned(),
            moved => format!("Lifted {moved} models off the plate"),
        });
    }
}

/// What the click has picked: how much of it, which groups it belongs to, and the
/// three things that can be done to it without the mouse.
fn editing(ui: &mut egui::Ui, window: &mut Window) {
    if window.tools.supports.picked.is_empty() {
        hint(
            ui,
            "Click a tip, a strut, a joint, the trunk or the foot to pick it. Drag to \
             carry it, shift adds, alt removes.",
        );
        return;
    }

    let held = window.tools.supports.picked.clone();
    let supports = held
        .iter()
        .map(|picked| (picked.id, picked.frozen))
        .collect::<std::collections::BTreeSet<_>>();
    stats(
        ui,
        &[
            ("Parts", held.len().to_string()),
            ("Supports", supports.len().to_string()),
        ],
    );

    ui.add_space(4.0);
    let active = window.tools.supports.active;
    if primary_button(ui, icon::SUPPORTS, "Move to the group in hand", true).clicked() {
        for (id, frozen) in &supports {
            if let Some(object) = window.doc.scene.get_mut(*id)
                && let Some(tree) = object.supports.frozen_mut(*frozen)
            {
                tree.set_group(active);
            }
        }
    }

    ui.add_space(4.0);
    if secondary_button(ui, icon::SUPPORTS, "Grow these again").clicked() {
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
    if secondary_button(ui, icon::REMOVE, "Remove these supports").clicked() {
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
        "Grow them again"
    } else {
        "Grow the supports"
    };
    let what = format!(
        "Cuts {} to find what hangs over nothing.",
        window.doc.scene.scope()
    );
    if primary_button(ui, icon::SUPPORTS, label, blocker.is_none())
        .on_hover_text(what)
        .clicked()
    {
        let started = window
            .tools
            .supports
            .start(&window.doc.scene, window.machine.slicing.layer_height_mm());
        window.machine.status.report("Placing supports", started);
    }
    if let Some(reason) = blocker {
        ui.vertical_centered(|ui| tone(ui, reason, theme::colors().text_low));
    }
}

/// The progress bar and the cancel button of the run that is going.
fn running(ui: &mut egui::Ui, job: &SupportJob) {
    tone(ui, &job.label(), theme::colors().text_mid);
    progress_bar(ui, job.fraction());

    ui.add_enabled_ui(!job.is_cancelling(), |ui| {
        if secondary_button(ui, icon::CANCEL, "Cancel").clicked() {
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
            ("Supports", points.to_string()),
            ("Standing", columns.to_string()),
        ],
    );

    if points == 0 {
        return;
    }
    if points != columns {
        ui.add_space(4.0);
        hint(
            ui,
            "Some supports have no room to stand and are not printed.",
        );
    }

    ui.add_space(4.0);
    if secondary_button(ui, icon::REMOVE, "Remove every support").clicked() {
        for object in window.doc.scene.targets_mut() {
            object.supports.clear();
        }
        // What was held is gone with the rest.
        window.tools.supports.picked.clear();
    }
}
