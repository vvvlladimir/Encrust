//! The printer and resin pickers, and the file dialogs the File menu opens a profile with.

use anyhow::Context as _;
use printer_profiles::{MaterialProfile, PrinterProfile};

use crate::import::recenter;
use crate::panels::Window;
use crate::plate::Plate;
use crate::settings::{installed, installed_printers};
use crate::slicing::Slicing;
use crate::state::Machine;
use crate::status::Status;

/// The catalogue's printers, then the way into the settings of the one in hand.
pub fn printer_menu(ui: &mut egui::Ui, window: &mut Window) {
    let chosen = window.machine.slicing.printer_id.clone();
    let mut picked: Option<(String, PrinterProfile)> = None;
    let mut listed = false;
    for entry in installed_printers(&window.machine.slicing.catalogue) {
        listed = true;
        let label = format!("{} {}", entry.profile.manufacturer, entry.profile.name);
        if ui
            .selectable_label(chosen.as_deref() == Some(entry.id.as_str()), label)
            .clicked()
        {
            picked = Some((entry.id.clone(), entry.profile.clone()));
        }
    }
    if !listed {
        ui.label("No machine yet. Add one in Settings.");
    }

    ui.separator();
    if ui.button("Printer settings...").clicked() {
        ui.close();
        let printer = window.machine.slicing.printer_id.clone();
        window
            .machine
            .settings
            .open(&window.machine.slicing.catalogue, printer.as_deref(), None);
        return;
    }
    if let Some((id, profile)) = picked {
        ui.close();
        apply_printer(window, profile, Some(id));
    }
}

/// The resins set up on the printer in hand, a printer's resins being its own presets,
/// then the way into the settings of the one in hand.
pub fn resin_menu(ui: &mut egui::Ui, machine: &mut Machine) {
    let slicing = &mut machine.slicing;
    let chosen = slicing.resin_id.clone();
    let printer_id = slicing.printer_id.clone();
    let mut picked: Option<(String, MaterialProfile)> = None;
    let mut listed = false;
    for entry in slicing.catalogue.resins() {
        let here = installed(&entry.source)
            && printer_id
                .as_deref()
                .is_none_or(|id| entry.profile.is_tuned_for(id));
        if !here {
            continue;
        }
        listed = true;
        let label = entry.profile.name.clone();
        if ui
            .selectable_label(chosen.as_deref() == Some(entry.id.as_str()), label)
            .clicked()
        {
            picked = Some((entry.id.clone(), entry.profile.clone()));
        }
    }
    if !listed {
        ui.label("No resins on this printer yet. Add them in Settings.");
    }

    ui.separator();
    if ui.button("Resin settings...").clicked() {
        ui.close();
        let (printer, resin) = (slicing.printer_id.clone(), slicing.resin_id.clone());
        machine
            .settings
            .open(&slicing.catalogue, printer.as_deref(), resin.as_deref());
        return;
    }
    if let Some((id, resin)) = picked {
        ui.close();
        slicing.resin_id = Some(id);
        slicing.set_material(resin);
    }
}

/// Loads a printer profile the user picks off disk.
pub fn open_printer(window: &mut Window) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("Printer profile", &["toml"])
        .pick_file()
    else {
        return;
    };

    let loaded = PrinterProfile::load(&path)
        .with_context(|| format!("cannot load the profile {}", path.display()));
    let Some(profile) = window
        .machine
        .status
        .report(&format!("Loaded {}", path.display()), loaded)
    else {
        return;
    };
    apply_printer(window, profile, None);
}

/// Stands the plate and everything on it under a new printer.
///
/// A new build volume moves the middle of the plate, so everything already imported is
/// put back over the new centre rather than left hanging off the old one. The machine on
/// the network follows the profile, because it is bound to it; see ADR 0156.
pub fn apply_printer(window: &mut Window, profile: PrinterProfile, id: Option<String>) {
    window.doc.plate = Plate::from_profile(&profile);
    window.machine.network.bind_to(id.clone());
    window.machine.slicing.set_printer(profile, id);
    for index in 0..window.doc.scene.objects().len() {
        recenter(&mut window.doc.scene, &window.doc.plate, index);
    }
    crate::panels::frame_view(
        &window.doc.scene,
        &window.doc.plate,
        &mut window.view.camera,
    );
}

/// Loads a resin profile, which brings the layer height it was measured at with it.
pub fn open_material(slicing: &mut Slicing, status: &mut Status) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("Resin profile", &["toml"])
        .pick_file()
    else {
        return;
    };

    let loaded = MaterialProfile::load(&path)
        .with_context(|| format!("cannot load the resin profile {}", path.display()));
    if let Some(material) = status.report(&format!("Loaded {}", path.display()), loaded) {
        slicing.resin_id = None;
        slicing.set_material(material);
    }
}
