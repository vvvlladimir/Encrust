//! The printer and resin pickers, and the File menu's imports of a machine or a resin.

use printer_profiles::{Kind, MaterialProfile, PrinterProfile};

use crate::files::{self, Handed, Wanted};
use crate::import::recenter;
use crate::panels::Window;
use crate::plate::BuildPlate;
use crate::settings::{installed, installed_printers};
use crate::state::Machine;

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
        ui.label("No machine yet. Add one in Machine and resin.");
    }

    ui.separator();
    if ui.button("Machine and resin...").clicked() {
        ui.close();
        crate::panels::open_machines(window.machine);
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
        ui.label("No resins on this printer yet. Add them in Machine and resin.");
    }

    ui.separator();
    if ui.button("Edit this resin...").clicked() {
        ui.close();
        let (printer, resin) = (slicing.printer_id.clone(), slicing.resin_id.clone());
        machine
            .settings
            .open_machines(&slicing.catalogue, printer.as_deref(), resin.as_deref());
        return;
    }
    if let Some((id, resin)) = picked {
        ui.close();
        slicing.resin_id = Some(id);
        slicing.set_material(resin);
    }
}

/// Asks for a machine's file, keeps it among the user's machines and stands the plate
/// under it.
pub fn open_printer(window: &mut Window) {
    if let Some(file) = files::pick(Wanted::PrinterProfile) {
        import_printer(window, &file);
    }
}

/// Keeps a machine the user handed over, and stands the plate under it.
pub fn import_printer(window: &mut Window, file: &Handed) {
    let Some(id) = crate::exchange::import_file(window.machine, Kind::Printer, file, None) else {
        return;
    };
    if let Ok(entry) = window.machine.slicing.catalogue.printer(&id) {
        let profile = entry.profile.clone();
        apply_printer(window, profile, Some(id));
    }
}

/// Stands the plate and everything on it under a new printer.
///
/// A new build volume moves the middle of the plate, so everything already imported is
/// put back over the new centre rather than left hanging off the old one. The printers
/// on the network follow the profile, because it lists them; see ADR 0265.
pub fn apply_printer(window: &mut Window, profile: PrinterProfile, id: Option<String>) {
    let plate = BuildPlate::from_profile(&profile);
    window.machine.network.bind_to(id.clone(), Some(&profile));
    learn_printer_names(window.machine);
    window.machine.slicing.set_printer(profile, id);
    stand_on(window, plate);
}

/// Takes the machine out from under the plate: the default envelope, nothing bound and
/// nothing to slice for. It is what removing the last machine leaves (BUG-14).
pub fn clear_printer(window: &mut Window) {
    window.machine.network.bind_to(None, None);
    window.machine.slicing.clear_printer();
    stand_on(window, BuildPlate::default());
}

/// Tells the network what every profile calls its printers, after a profile changed.
pub fn learn_printer_names(machine: &mut crate::state::Machine) {
    let profiles = machine
        .slicing
        .catalogue
        .printers()
        .map(|entry| &entry.profile);
    machine.network.learn_names(profiles);
}

/// Stands the plate and everything on it on a new build volume.
fn stand_on(window: &mut Window, plate: BuildPlate) {
    window.doc.plate = plate;
    for index in 0..window.doc.scene.objects().len() {
        recenter(&mut window.doc.scene, &window.doc.plate, index);
    }
    crate::panels::frame_view(
        &window.doc.scene,
        &window.doc.plate,
        &mut window.view.camera,
    );
}

/// Asks for a resin's file, keeps it on the machine in hand and prints with it.
pub fn open_material(machine: &mut Machine) {
    if let Some(file) = files::pick(Wanted::ResinProfile) {
        import_material(machine, &file);
    }
}

/// Keeps a resin the user handed over, set up on the machine in hand, and prints with it.
pub fn import_material(machine: &mut Machine, file: &Handed) {
    let printer = machine.slicing.printer_id.clone();
    let Some(id) = crate::exchange::import_file(machine, Kind::Resin, file, printer.as_deref())
    else {
        return;
    };
    let slicing = &mut machine.slicing;
    if let Ok(entry) = slicing.catalogue.resin(&id) {
        let resin = entry.profile.clone();
        slicing.resin_id = Some(id);
        slicing.set_material(resin);
    }
}
