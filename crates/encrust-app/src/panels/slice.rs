use anyhow::Context as _;
use core_engine::{NameFields, file_stem};
use printer_profiles::Connection;

use crate::i18n::tr;
use crate::job::SlicedFormat;
#[cfg(not(target_arch = "wasm32"))]
use crate::job::{applied_to, label_of};
use crate::network::{Reach, temporary_path};
use crate::panels::{Window, destination};
use crate::prefs::General;
use crate::scene::Scene;
use crate::slicing::Slicing;
use crate::state::Machine;
use crate::status::Status;
use crate::ui::{busy, companion_square, hint, icon, primary_button, theme};

/// What the keyboard shortcut and the File menu do: this plate into a file, refused for the
/// same reasons the press is greyed out.
pub fn slice_this_plate(window: &mut Window) {
    slice_to_file(window, Run::ThisPlate);
}

/// Every plate of the project, each into a file of its own beside the one the user names.
pub fn slice_every_plate(window: &mut Window) {
    slice_to_file(window, Run::EveryPlate);
}

fn slice_to_file(window: &mut Window, run: Run) {
    if !can_slice(window) {
        return;
    }
    start(
        &window.doc.scene,
        &mut window.machine.slicing,
        &mut window.machine.status,
        run,
        Via::File,
        &window.machine.general,
    );
}

/// Whether a slice into a file may start: the press, the File menu and the key ask alike.
pub fn can_slice(window: &Window) -> bool {
    let slicing = &window.machine.slicing;
    slicing.job.is_none() && slicing.blocker(&window.doc.scene).is_none()
}

/// The plate card's primary action and its square: Slice and send, its bar and Cancel, then
/// Start print; Slice to a file alone without a machine to take it (ADR 0348, 0382).
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    let blocker = window.machine.slicing.blocker(&window.doc.scene);
    let free = can_slice(window);
    let bound = bound_machine(window.machine);
    let slot = bound
        .as_ref()
        .and_then(|bound| send_slot(window.machine, bound));
    let label = tr!(
        "slice-to",
        extension = window.machine.slicing.format.extension()
    );
    let tooltip = tr!("slice-tooltip");
    let file_tip = blocker.as_deref().unwrap_or(&tooltip);
    let press = match &slot {
        Some(SendSlot {
            step: SendStep::Running(fraction),
            name,
        }) => {
            if running(ui, window.machine, name, *fraction) {
                cancel_send(window.machine);
            }
            None
        }
        Some(slot) => paired(ui, slot, (free, blocker.as_deref()), (&label, file_tip)),
        None => primary_button(ui, &label, free)
            .on_hover_text(file_tip)
            .clicked()
            .then_some(Press::Slice(Via::File)),
    };
    match press {
        Some(Press::Slice(via)) => start(
            &window.doc.scene,
            &mut window.machine.slicing,
            &mut window.machine.status,
            Run::ThisPlate,
            via,
            &window.machine.general,
        ),
        Some(Press::StartPrint) => window
            .machine
            .network
            .start_print(&mut window.machine.status),
        None => {}
    }
    let saved_instead = bound
        .filter(|_| slot.is_none() && blocker.is_none())
        .and_then(|bound| bound.blocker)
        .map(|why| tr!("slice-saved-instead", why = why));
    // An empty plate is said so in the models card; the button's tooltip still tells why.
    let scene = &window.doc.scene;
    let printable = scene.has_printable(scene.active_plate());
    if let Some(why) = blocker.filter(|_| printable).or(saved_instead) {
        ui.add_space(theme::NOTE_GAP);
        hint(ui, &why);
    }
}

/// The primary that sends or starts the print, and Slice to a file the square beside it,
/// as tall. `label` and `tip` are the square's; answers what was pressed.
fn paired(
    ui: &mut egui::Ui,
    slot: &SendSlot,
    (free, blocker): (bool, Option<&str>),
    (label, tip): (&str, &str),
) -> Option<Press> {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme::BUTTON_GAP;
        let width = (ui.available_width() - theme::PRIMARY_H - theme::BUTTON_GAP).max(0.0);
        let sent = ui
            .allocate_ui(egui::vec2(width, theme::PRIMARY_H), |ui| {
                send_button(ui, slot, free, blocker)
            })
            .inner;
        let file = companion_square(ui, icon::SLICE_TO_FILE, label, theme::PRIMARY_H, free);
        let file = file.on_hover_text(tip).clicked();
        sent.or(file.then_some(Press::Slice(Via::File)))
    })
    .inner
}

/// The errand running for the bound machine: the slice, then the upload, its share and the
/// way to stop it. Answers whether Cancel was pressed.
fn running(ui: &mut egui::Ui, machine: &Machine, name: &str, fraction: Option<f32>) -> bool {
    let label = if machine.slicing.is_sending() {
        tr!("slice-slicing")
    } else {
        tr!("network-sending", name = name.to_owned())
    };
    let share = fraction.map(|share| (format!("{:.0}%", share * 100.0), share));
    let step = share
        .as_ref()
        .map(|(percent, share)| (percent.as_str(), *share));
    busy(ui, &label, step, true)
}

/// Stops whichever half of the errand is running: the slice, or the upload after it.
fn cancel_send(machine: &Machine) {
    match (&machine.slicing.job, &machine.network.job) {
        (Some(job), _) if machine.slicing.is_sending() => job.cancel(),
        (_, Some(job)) => job.cancel(),
        _ => {}
    }
}

/// Slice and send, or Start print once the file has landed.
fn send_button(
    ui: &mut egui::Ui,
    slot: &SendSlot,
    free: bool,
    blocker: Option<&str>,
) -> Option<Press> {
    let (label, tooltip, enabled, press) = match slot.step {
        SendStep::Start => (
            tr!("slice-start-print"),
            tr!("slice-start-tooltip", name = slot.name.clone()),
            true,
            Press::StartPrint,
        ),
        SendStep::Offer | SendStep::Running(_) => (
            tr!("slice-send"),
            tr!("slice-send-tooltip"),
            free && slot.step == SendStep::Offer,
            Press::Slice(Via::Printer),
        ),
    };
    let hover = match slot.step {
        SendStep::Offer => blocker.unwrap_or(&tooltip),
        _ => &tooltip,
    };
    primary_button(ui, &label, enabled)
        .on_hover_text(hover)
        .clicked()
        .then_some(press)
}

/// What the plate's card offers the bound machine, and how far that errand has gone.
struct SendSlot {
    name: String,
    step: SendStep,
}

#[derive(Clone, Copy, PartialEq)]
enum SendStep {
    /// Slice and send: the machine answered and is free.
    Offer,
    /// Slicing for it, uploading, or starting; the share done when the job counts.
    Running(Option<f32>),
    /// The file landed and waits for a deliberate press to print.
    Start,
}

/// What the card's buttons were pressed for.
enum Press {
    Slice(Via),
    StartPrint,
}

fn send_slot(machine: &Machine, bound: &Bound) -> Option<SendSlot> {
    let network = &machine.network;
    let running = match (&machine.slicing.job, &network.job) {
        (Some(job), _) if machine.slicing.is_sending() => Some(job.fraction()),
        (_, Some(job)) => Some(job.fraction()),
        _ => None,
    };
    Some(SendSlot {
        step: step(bound, running, network.sent.is_some())?,
        name: bound.name.clone(),
    })
}

/// The send slot's step: an errand running wins, then a file waiting to print, then an
/// offer to a machine that answered and is free, for one not asked yet may be switched off.
fn step(bound: &Bound, running: Option<Option<f32>>, sent: bool) -> Option<SendStep> {
    match running {
        Some(fraction) => Some(SendStep::Running(fraction)),
        None if sent => Some(SendStep::Start),
        None => {
            (bound.reach == Reach::Answered && bound.blocker.is_none()).then_some(SendStep::Offer)
        }
    }
}

/// The machine the printer in hand is bound to, as the row needs it: a profile that
/// states no connection, or names no machine, offers no sending at all.
struct Bound {
    name: String,
    reach: Reach,
    /// Why this machine cannot take what the window is set to write, or not now.
    blocker: Option<String>,
}

/// The bound machine once found, the network kept fresh by the printer row over the press.
fn bound_machine(machine: &Machine) -> Option<Bound> {
    let connection = destination::connection(machine);
    let network = &machine.network;
    if connection == Connection::None {
        return None;
    }
    let listed = network.chosen_machine()?;
    let target = network.resolve(listed)?;
    if !target.speaks(connection) {
        return None;
    }
    let name = network.label(&target);
    let sending = network
        .job
        .as_ref()
        .map(|_| tr!("network-sending", name = name.clone()));
    Some(Bound {
        reach: network.reach(&target),
        blocker: sending
            .or_else(|| network.blocker(machine.slicing.format))
            .or_else(|| network.busy()),
        name,
    })
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

/// Slices into a file the user names, the dialog opening on the name `general` makes and in
/// its folder when there is one.
fn start(
    scene: &Scene,
    slicing: &mut Slicing,
    status: &mut Status,
    run: Run,
    via: Via,
    general: &General,
) {
    let format = slicing.format;
    let name = file_name(scene, slicing, &general.file_name);
    if via == Via::Printer {
        let path = temporary_path(&name);
        slicing.send_when_written();
        let started = slicing
            .start(scene, scene.active_plate(), path)
            .context(tr!("slice-cannot-send"));
        let _ = status.report(&tr!("slice-slicing"), started);
        return;
    }
    let folder = general.sliced_dir.as_deref();
    let Some(path) = save_path(&name, format, slicing.printer.as_ref(), folder) else {
        return;
    };

    if run == Run::EveryPlate {
        let queued = slicing.start_all(scene, &path, status);
        if queued > 0 && !status.is_error() {
            *status = Status::Info(tr!(
                "slice-all-started",
                count = queued,
                path = path.display().to_string()
            ));
        }
        return;
    }

    let started = slicing
        .start(scene, scene.active_plate(), path.clone())
        .with_context(|| tr!("slice-cannot-slice-into", path = path.display().to_string()));
    let _ = status.report(
        &tr!("slice-slicing-into", path = path.display().to_string()),
        started,
    );
}

#[cfg(not(target_arch = "wasm32"))]
fn save_path(
    name: &str,
    format: SlicedFormat,
    printer: Option<&printer_profiles::PrinterProfile>,
    folder: Option<&std::path::Path>,
) -> Option<std::path::PathBuf> {
    // The chosen format goes first, so the dialog offers it; the others this machine can
    // be written into stay reachable, because the name is what decides in the end.
    let mut dialog = rfd::FileDialog::new().set_file_name(name);
    if let Some(folder) = folder {
        dialog = dialog.set_directory(folder);
    }
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
    name: &str,
    _format: SlicedFormat,
    _printer: Option<&printer_profiles::PrinterProfile>,
    _folder: Option<&std::path::Path>,
) -> Option<std::path::PathBuf> {
    Some(name.into())
}

/// What a file sliced now is called: `template` over the selected model, or the first one,
/// the machine, the resin and the plate in hand, with the chosen format's extension.
pub fn file_name(scene: &Scene, slicing: &Slicing, template: &str) -> String {
    let name = scene
        .selected()
        .and_then(|id| scene.get(id))
        .or_else(|| scene.objects().first())
        .map_or("model", |object| object.name.as_str());
    let (model, model_ext) = split_mesh_extension(name);
    let fields = NameFields {
        model,
        model_ext,
        printer: slicing.printer.as_ref().map_or("", |printer| &printer.name),
        resin: &slicing.material.name,
        layer_height_mm: slicing.layer_height_mm(),
        plate: scene.active_plate() + 1,
        when: jiff::Zoned::now().datetime(),
    };
    let stem = file_stem(template, &fields);
    format!("{stem}.{}", slicing.format.extension())
}

/// A model's name, which keeps the extension of the file it came from, and that extension
/// apart; a name ending in no mesh extension is all name.
fn split_mesh_extension(name: &str) -> (&str, &str) {
    match name.rsplit_once('.') {
        Some((stem, ext))
            if !stem.is_empty()
                && crate::files::MESHES
                    .iter()
                    .any(|mesh| mesh.eq_ignore_ascii_case(ext)) =>
        {
            (stem, ext)
        }
        _ => (name, ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::ImportSummary;
    use core_engine::DEFAULT_TEMPLATE;
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
        let scene = scene_with_a_model("bracket.stl");
        let mut slicing = Slicing::default();
        slicing.format = SlicedFormat::Goo;
        assert_eq!(
            file_name(&scene, &slicing, DEFAULT_TEMPLATE),
            "bracket.goo",
            "the model's own extension is not part of its name"
        );
        assert_eq!(
            file_name(&scene, &slicing, "{model}_{ext}"),
            "bracket_stl.goo"
        );
        assert_eq!(
            file_name(&Scene::default(), &slicing, DEFAULT_TEMPLATE),
            "model.goo"
        );
        slicing.format = SlicedFormat::Ctb(core_pipeline::CtbVersion::V5);
        assert_eq!(
            file_name(&scene, &slicing, DEFAULT_TEMPLATE),
            "bracket.ctb",
            "the chosen format names the file the dialog opens on"
        );
    }

    #[test]
    fn the_name_follows_the_template_written_in_settings() {
        let scene = scene_with_a_model("bracket.v2");
        let mut slicing = Slicing::default();
        slicing.format = SlicedFormat::Goo;
        slicing.material.name = "Grey".to_owned();
        slicing.material.layer_height_mm = 0.05;
        assert_eq!(
            file_name(&scene, &slicing, "{model}_{resin}_{layer}um_p{plate}"),
            "bracket.v2_Grey_50um_p1.goo",
            "a dot that ends no mesh extension stays in the name"
        );
    }

    fn bound(reach: Reach, blocker: Option<&str>) -> Bound {
        Bound {
            name: "Bench A".to_owned(),
            reach,
            blocker: blocker.map(str::to_owned),
        }
    }

    #[test]
    fn only_a_free_machine_that_answered_offers_sending() {
        let offer = |bound: &Bound| step(bound, None, false) == Some(SendStep::Offer);
        assert!(offer(&bound(Reach::Answered, None)));
        assert!(
            !offer(&bound(Reach::Unasked, None)),
            "a machine not asked yet may be switched off"
        );
        assert!(!offer(&bound(Reach::Silent, None)));
        assert!(step(&bound(Reach::Answered, Some("printing")), None, false).is_none());
    }

    #[test]
    fn a_running_errand_shows_its_bar_and_a_landed_file_offers_start() {
        let busy = bound(Reach::Answered, Some("sending"));
        assert!(step(&busy, Some(Some(0.5)), false) == Some(SendStep::Running(Some(0.5))));
        assert!(
            step(&busy, Some(None), true) == Some(SendStep::Running(None)),
            "starting the print runs too"
        );
        assert!(step(&bound(Reach::Answered, None), None, true) == Some(SendStep::Start));
    }
}
