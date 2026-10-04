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
use crate::ui::{companion_button, icon, primary_button, secondary_button, theme, tone};

use super::estimate;

/// The footer is a block at the foot of the column rather than the end of the scroll, so
/// the one action the window is for is always in the same corner. The estimate stands
/// over it, because what the stack comes to is what decides whether to press the button.
///
/// This is only what the panel opens at: it takes the height of its contents from there.
pub const IDLE_H: f32 = 138.0;

/// Width of the Send button beside Slice, and of the dot that says whether its machine
/// answered.
const SEND_W: f32 = 112.0;
const DOT_W: f32 = 14.0;

pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    egui::Frame::new()
        .inner_margin(theme::PANEL_MARGIN)
        .show(ui, |ui| {
            estimate::row(ui, window);
            ui.add_space(6.0);
            if let Some(job) = window.machine.network.job.as_ref() {
                if running(ui, &job.label(), job.fraction(), job.is_cancelling()) {
                    job.cancel();
                }
                return;
            }
            match window.machine.slicing.job.as_ref() {
                Some(job) => {
                    if running(ui, &job.label(), job.fraction(), job.is_cancelling()) {
                        job.cancel();
                    }
                }
                None => action(ui, window),
            }
        });
}

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

fn action(ui: &mut egui::Ui, window: &mut Window) {
    let blocker = window.machine.slicing.blocker(&window.doc.scene);
    // Nothing on the plate means nothing to start either: the row would outlive what
    // the user was looking at when they sent it.
    if blocker.is_none() {
        waiting_to_print(ui, window);
    }
    if let Some(via) = slice_row(ui, window.machine, blocker) {
        start(
            &window.doc.scene,
            &mut window.machine.slicing,
            &mut window.machine.status,
            Run::ThisPlate,
            via,
        );
    }

    // A project of one plate has nothing to offer here, and the button would only ask a
    // question with one answer.
    let plates = window.doc.scene.plates().len();
    if plates > 1 {
        ui.add_space(4.0);
        let label = format!("Slice all {plates} plates");
        if secondary_button(ui, icon::SLICE, &label).clicked() {
            start(
                &window.doc.scene,
                &mut window.machine.slicing,
                &mut window.machine.status,
                Run::EveryPlate,
                Via::File,
            );
        }
    }
}

/// A stack that reached the printer is not printing yet: starting it is a second,
/// deliberate press, because nobody is standing at the machine when the first one lands.
fn waiting_to_print(ui: &mut egui::Ui, window: &mut Window) {
    let Some(sent) = window.machine.network.sent.clone() else {
        return;
    };
    let hint = format!("{} is on {}", sent.filename, sent.printer);
    if secondary_button(ui, icon::PLAY, "Start print")
        .on_hover_text(hint)
        .clicked()
    {
        window
            .machine
            .network
            .start_print(&mut window.machine.status);
    }
    ui.add_space(4.0);
}

/// Slicing and sending both report the same three things, so they are drawn the same
/// way: what is happening, how far along, and a way out. Answers whether to stop.
fn running(ui: &mut egui::Ui, label: &str, fraction: Option<f32>, cancelling: bool) -> bool {
    progress(ui, label, fraction);
    ui.add_space(4.0);
    let mut cancel = false;
    ui.add_enabled_ui(!cancelling, |ui| {
        cancel = secondary_button(ui, icon::CANCEL, "Cancel").clicked();
    });
    cancel
}

/// What is running and how far along, on one line over the bar. The line is cut rather
/// than wrapped: the block has a fixed height and a printer can be called anything.
fn progress(ui: &mut egui::Ui, label: &str, fraction: Option<f32>) {
    let colors = theme::colors();
    ui.horizontal(|ui| {
        if let Some(fraction) = fraction {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                tone(ui, &format!("{:.0} %", fraction * 100.0), colors.text_high);
                one_line(ui, label, colors.text_mid);
            });
            return;
        }
        one_line(ui, label, colors.text_mid);
    });
    let bar = match fraction {
        Some(fraction) => egui::ProgressBar::new(fraction),
        // Slicing reports no layer counts, so the bar animates rather than lies.
        None => egui::ProgressBar::new(0.0).animate(true),
    };
    ui.add(bar.desired_height(6.0).corner_radius(theme::R_CONTROL));
}

fn one_line(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    let text = egui::RichText::new(text).font(theme::small()).color(color);
    ui.add(egui::Label::new(text).truncate());
}

/// Slice to a file, and beside it the machine this printer profile is bound to, with
/// whether it answered the last scan. See `docs/decisions/0157`.
fn slice_row(
    ui: &mut egui::Ui,
    machine: &mut Machine,
    blocker: Option<&'static str>,
) -> Option<Via> {
    let bound = bound_machine(machine);
    let label = format!("Slice to .{}", machine.slicing.format.extension());
    let hint = blocker.unwrap_or("Rasterise the plate into a printable file");
    let mut pressed = None;
    let mut rescan = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let beside = match bound {
            Some(_) => SEND_W + DOT_W + 8.0,
            None => 0.0,
        };
        let width = ui.available_width() - beside;
        ui.allocate_ui(egui::vec2(width, theme::PRIMARY_H), |ui| {
            if primary_button(ui, icon::SLICE, &label, blocker.is_none())
                .on_hover_text(hint)
                .clicked()
            {
                pressed = Some(Via::File);
            }
        });
        let Some(bound) = &bound else {
            return;
        };
        rescan = reach_dot(ui, bound).clicked();
        let enabled = blocker.is_none() && bound.blocker.is_none();
        let hint = bound.blocker.map_or_else(
            || format!("Slice and send to {}", bound.name),
            str::to_owned,
        );
        ui.allocate_ui(egui::vec2(SEND_W, theme::PRIMARY_H), |ui| {
            if companion_button(ui, icon::SEND, "Send", enabled)
                .on_hover_text(hint)
                .clicked()
            {
                pressed = Some(Via::Printer);
            }
        });
    });
    if rescan {
        machine.network.scan();
    }
    pressed
}

/// The machine the printer in hand is bound to, as the row needs it: a profile that
/// states no connection, or names no machine, has no Send button at all.
struct Bound {
    name: String,
    reach: Reach,
    /// Why this machine cannot take what the window is set to write.
    blocker: Option<&'static str>,
}

fn bound_machine(machine: &mut Machine) -> Option<Bound> {
    // TODO(step-A6): a browser reaches no printer yet; it sends over `fetch` and WebSocket.
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

/// Whether the bound machine answered, as a dot that asks again when it is pressed.
fn reach_dot(ui: &mut egui::Ui, bound: &Bound) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(DOT_W, theme::PRIMARY_H), egui::Sense::click());
    let colors = theme::colors();
    let colour = match bound.reach {
        Reach::Answered => colors.ok,
        Reach::Silent => colors.danger,
        Reach::Unasked => colors.text_low,
    };
    ui.painter().circle_filled(rect.center(), 4.0, colour);
    let hint = format!("{}. Press to scan again", bound.reach.hint(&bound.name));
    response.on_hover_text(hint)
}

/// Where what the press writes goes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Via {
    File,
    Printer,
}

/// Whether the Slice button cuts what is in front of the user or the whole project.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Run {
    ThisPlate,
    EveryPlate,
}

fn start(scene: &Scene, slicing: &mut Slicing, status: &mut Status, run: Run, via: Via) {
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
    let Some(path) = save_path(scene, format) else {
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
fn save_path(scene: &Scene, format: SlicedFormat) -> Option<std::path::PathBuf> {
    // The chosen format goes first, so the dialog offers it; the others stay reachable,
    // because the name is what decides in the end.
    let mut dialog = rfd::FileDialog::new().set_file_name(default_file_name(scene, format));
    for choice in [format].into_iter().chain(SlicedFormat::CHOICES) {
        dialog = dialog.add_filter(label_of(choice), &[choice.extension()]);
    }
    Some(applied_to(format, dialog.save_file()?))
}

/// A browser asks where a download goes itself, so the file is only named.
#[cfg(target_arch = "wasm32")]
fn save_path(scene: &Scene, format: SlicedFormat) -> Option<std::path::PathBuf> {
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
            default_file_name(&scene, SlicedFormat::Ctb(format_chitu::CtbVersion::V5)),
            "bracket.ctb",
            "the chosen format names the file the dialog opens on"
        );
        assert_eq!(
            default_file_name(&Scene::default(), SlicedFormat::Goo),
            "model.goo"
        );
    }
}
