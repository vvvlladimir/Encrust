use crate::job::SupportJob;
use crate::panels::Window;
use crate::panels::support_fields::Group;
use crate::state::Doc;
use crate::status::Status;
use crate::ui::{
    describe, hint, icon, later, primary_button, progress_bar, secondary_button, section, stats,
    theme, tone,
};

/// Growing supports under whatever hangs over nothing, and the parts of those already
/// standing, which a click in this tool picks.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    section(ui, "Islands", None, |ui| {
        describe(
            ui,
            "Layer parts with nothing under them. They cure loose in the vat.",
        );
        hint(
            ui,
            "Not scanned yet. Check finds them once the layers are cut.",
        );
        // TODO(step-8): scan the plate for islands before it is sliced, and count the ones
        // the supports hold.
        later(ui, |ui| secondary_button(ui, icon::CHECK, "Scan"));
    });
    section(ui, "Where", None, |ui| {
        Group::Automatic.show(ui, &mut window.tools.supports.profile);
        lift(ui, window);
    });
    section(ui, "Picked", None, |ui| editing(ui, window));
    section(ui, "Already done", None, |ui| placed(ui, window.doc));
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
        for (id, frozen) in supports.iter().rev() {
            if let Some(object) = window.doc.scene.get_mut(*id) {
                object.supports.remove_frozen(*frozen);
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
    if primary_button(ui, icon::SUPPORTS, label, blocker.is_none()).clicked() {
        let started = window
            .tools
            .supports
            .start(&window.doc.scene, window.machine.slicing.layer_height_mm());
        window.machine.status.report("Placing supports", started);
    }

    let scope = format!(
        "Cuts {} to find what hangs over nothing.",
        window.doc.scene.scope()
    );
    let text = blocker.unwrap_or(&scope);
    ui.vertical_centered(|ui| tone(ui, text, theme::colors().text_low));
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
fn placed(ui: &mut egui::Ui, doc: &mut Doc) {
    let (points, columns) = standing(doc);

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
        for object in doc.scene.targets_mut() {
            object.supports.clear();
        }
    }
}
