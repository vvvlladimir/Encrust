use anyhow::Context as _;
use printer_profiles::Connection;

use crate::job::SlicedFormat;
#[cfg(not(target_arch = "wasm32"))]
use crate::job::{applied_to, label_of};
use crate::network::{Reach, temporary_path};
use crate::panels::Window;
use crate::scene::Scene;
use crate::slicing::Slicing;
use crate::state::Machine;
use crate::status::Status;
use crate::ui::{companion_button, icon, primary_button, theme, tone};

/// The dot that says whether the bound machine answered.
const DOT_W: f32 = 14.0;

/// What the keyboard shortcut does: the primary button's own action, a file, refused for
/// the same reasons that button is greyed out.
pub fn slice_this_plate(window: &mut Window) {
    if window.machine.slicing.job.is_some()
        || window.machine.slicing.blocker(&window.doc.scene).is_some()
    {
        return;
    }
    start(
        &window.doc.scene,
        &mut window.machine.slicing,
        &mut window.machine.status,
        Run::ThisPlate,
        Via::File,
    );
}

/// The one action the window is for, at the foot of the plate column: Slice, and beside it
/// a caret for every plate at once and for the bound machine.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    let blocker = window.machine.slicing.blocker(&window.doc.scene);
    // Nothing on the plate means nothing to start either.
    if blocker.is_none() {
        broken_reminder(ui, &window.doc.scene);
    }
    let busy = window.machine.slicing.job.is_some() || window.machine.network.job.is_some();
    let bound = bound_machine(window.machine);
    let plates = window.doc.scene.plates().len();
    let more = bound.is_some() || plates > 1;

    let mut pressed = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let beside = if more { theme::PRIMARY_H + 2.0 } else { 0.0 };
        let label = format!("Slice to .{}", window.machine.slicing.format.extension());
        let hint = blocker.unwrap_or("Rasterise the plate into a printable file");
        ui.allocate_ui(
            egui::vec2(ui.available_width() - beside, theme::PRIMARY_H),
            |ui| {
                let enabled = blocker.is_none() && !busy;
                if primary_button(ui, icon::SLICE, &label, enabled)
                    .on_hover_text(hint)
                    .clicked()
                {
                    pressed = Some((Run::ThisPlate, Via::File));
                }
            },
        );
        if more {
            pressed = pressed.or(more_menu(ui, window, bound.as_ref(), blocker, busy));
        }
    });
    if let Some((run, via)) = pressed {
        start(
            &window.doc.scene,
            &mut window.machine.slicing,
            &mut window.machine.status,
            run,
            via,
        );
    }
}

/// The caret beside Slice: every plate at once, and the machine this printer is bound
/// to, with whether it answered. See `docs/decisions/0157`.
fn more_menu(
    ui: &mut egui::Ui,
    window: &mut Window,
    bound: Option<&Bound>,
    blocker: Option<&'static str>,
    busy: bool,
) -> Option<(Run, Via)> {
    let response = ui
        .allocate_ui(egui::vec2(theme::PRIMARY_H, theme::PRIMARY_H), |ui| {
            companion_button(ui, "", icon::CARET_DOWN, true)
        })
        .inner
        .on_hover_text("Slice every plate, or slice and send");
    let mut pressed = None;
    let mut rescan = false;
    egui::Popup::menu(&response).show(|ui| {
        let plates = window.doc.scene.plates().len();
        let free = blocker.is_none() && !busy;
        if plates > 1
            && ui
                .add_enabled(
                    free,
                    egui::Button::new(format!("Slice all {plates} plates")),
                )
                .clicked()
        {
            pressed = Some((Run::EveryPlate, Via::File));
        }
        let Some(bound) = bound else {
            return;
        };
        let hint = bound.blocker.map_or_else(
            || format!("Slice and send to {}", bound.name),
            str::to_owned,
        );
        let send = egui::Button::new(format!("{}  Slice and send to {}", icon::SEND, bound.name));
        if ui
            .add_enabled(free && bound.blocker.is_none(), send)
            .on_hover_text(&hint)
            .on_disabled_hover_text(blocker.unwrap_or(&hint))
            .clicked()
        {
            pressed = Some((Run::ThisPlate, Via::Printer));
        }
        ui.horizontal(|ui| {
            reach_dot(ui, bound.reach);
            rescan = ui
                .button("Scan again")
                .on_hover_text(bound.reach.hint(&bound.name))
                .clicked();
        });
    });
    if rescan {
        window.machine.network.scan();
    }
    pressed
}

/// A model left broken is a reminder here rather than a blocker: what is sliced from it
/// may be nothing like the model, and this is the last place to turn back.
pub(super) fn broken_reminder(ui: &mut egui::Ui, scene: &Scene) {
    let broken = crate::repair::broken_on_the_plate(scene);
    if broken == 0 {
        return;
    }
    let line = match broken {
        1 => "1 model on this plate is broken; what is sliced from it may not print".to_owned(),
        count => {
            format!(
                "{count} models on this plate are broken; what is sliced from them may not print"
            )
        }
    };
    tone(ui, &line, theme::colors().warn);
    ui.add_space(4.0);
}

/// The machine the printer in hand is bound to, as the row needs it: a profile that
/// states no connection, or names no machine, offers no sending at all.
pub(super) struct Bound {
    pub name: String,
    pub reach: Reach,
    /// Why this machine cannot take what the window is set to write.
    pub blocker: Option<&'static str>,
}

pub(super) fn bound_machine(machine: &mut Machine) -> Option<Bound> {
    // A browser reaches no printer, so the file is only ever downloaded (ADR 0182).
    if cfg!(target_arch = "wasm32") {
        return None;
    }
    let connection = machine
        .slicing
        .printer
        .as_ref()
        .map_or(Connection::None, |profile| profile.connection);
    if connection == Connection::None || machine.network.target().is_none() {
        return None;
    }
    machine.network.ensure_asked();

    let network = &machine.network;
    let target = network.target()?;
    target.speaks(connection).then(|| Bound {
        name: target.name(),
        reach: network.reach(&target),
        blocker: network.blocker(machine.slicing.format),
    })
}

/// Whether the bound machine answered the last scan, as a dot.
pub(super) fn reach_dot(ui: &mut egui::Ui, reach: Reach) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(DOT_W, DOT_W), egui::Sense::hover());
    let colors = theme::colors();
    let colour = match reach {
        Reach::Answered => colors.ok,
        Reach::Silent => colors.danger,
        Reach::Unasked => colors.text_low,
    };
    ui.painter().circle_filled(rect.center(), 4.0, colour);
}

/// Where what the press writes goes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Via {
    File,
    Printer,
}

/// Whether the Slice button cuts what is in front of the user or the whole project.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Run {
    ThisPlate,
    EveryPlate,
}

pub(super) fn start(scene: &Scene, slicing: &mut Slicing, status: &mut Status, run: Run, via: Via) {
    let format = slicing.format;
    if via == Via::Printer {
        let path = temporary_path(&default_file_name(scene, format));
        slicing.send_when_written();
        let started = slicing
            .start(scene, scene.active_plate(), path)
            .context("cannot slice the stack to send");
        let _ = status.report("Slicing", started);
        return;
    }
    let Some(path) = save_path(scene, format, slicing.printer.as_ref()) else {
        return;
    };

    if run == Run::EveryPlate {
        let queued = slicing.start_all(scene, &path, status);
        if queued > 0 && !status.is_error() {
            *status = Status::Info(format!("Slicing {queued} plates beside {}", path.display()));
        }
        return;
    }

    let started = slicing
        .start(scene, scene.active_plate(), path.clone())
        .with_context(|| format!("cannot slice into {}", path.display()));
    let _ = status.report(&format!("Slicing into {}", path.display()), started);
}

#[cfg(not(target_arch = "wasm32"))]
fn save_path(
    scene: &Scene,
    format: SlicedFormat,
    printer: Option<&printer_profiles::PrinterProfile>,
) -> Option<std::path::PathBuf> {
    // The chosen format goes first, so the dialog offers it; the others this machine can
    // be written into stay reachable, because the name is what decides in the end.
    let mut dialog = rfd::FileDialog::new().set_file_name(default_file_name(scene, format));
    let offered = printer.map_or_else(
        || SlicedFormat::CHOICES.to_vec(),
        |printer| SlicedFormat::choices_for(printer).collect(),
    );
    for choice in [format].into_iter().chain(offered) {
        dialog = dialog.add_filter(label_of(choice), &[choice.extension()]);
    }
    Some(applied_to(format, dialog.save_file()?))
}

/// A browser asks where a download goes itself, so the file is only named.
#[cfg(target_arch = "wasm32")]
fn save_path(
    scene: &Scene,
    format: SlicedFormat,
    _printer: Option<&printer_profiles::PrinterProfile>,
) -> Option<std::path::PathBuf> {
    Some(default_file_name(scene, format).into())
}

/// The selected model's name, or the first one's, so the dialog opens on something
/// recognisable.
fn default_file_name(scene: &Scene, format: SlicedFormat) -> String {
    let named = scene
        .selected()
        .and_then(|id| scene.get(id))
        .or_else(|| scene.objects().first());
    let stem = named.map_or("model", |object| object.name.as_str());
    format!("{stem}.{}", format.extension())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::ImportSummary;
    use core_geometry::{Mesh, Orientation, Transform, Vec3, diagnose};

    fn scene_with_a_model(name: &str) -> Scene {
        let mesh = Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
            ],
            vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
        );
        let mut scene = Scene::default();
        scene.insert(crate::scene::Imported::new(
            name.to_owned(),
            std::sync::Arc::new(mesh.clone()),
            Transform::default(),
            ImportSummary {
                vertices_merged: 0,
                faces_removed: 0,
                orientation: Orientation {
                    flipped_faces: 0,
                    inverted_shells: 0,
                    orientable: true,
                },
                diagnostics: diagnose(&mesh),
            },
        ));
        scene
    }

    #[test]
    fn the_save_dialog_opens_on_the_selected_models_name() {
        let scene = scene_with_a_model("bracket");
        assert_eq!(default_file_name(&scene, SlicedFormat::Goo), "bracket.goo");
        assert_eq!(
            default_file_name(&scene, SlicedFormat::Ctb(core_pipeline::CtbVersion::V5)),
            "bracket.ctb",
            "the chosen format names the file the dialog opens on"
        );
        assert_eq!(
            default_file_name(&Scene::default(), SlicedFormat::Goo),
            "model.goo"
        );
    }
}
