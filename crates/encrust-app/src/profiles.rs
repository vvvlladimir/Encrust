//! The printer and resin pickers, and the File menu's imports of a machine or a resin.

use printer_profiles::{Kind, MaterialProfile, PrinterProfile};

use crate::files::{self, Handed, Wanted};
use crate::i18n::tr;
use crate::import::recenter;
use crate::job::SlicedFormat;
use crate::network::{Network, Reach};
use crate::panels::Window;
use crate::plate::BuildPlate;
use crate::settings::{Library, installed, installed_printers};
use crate::state::Machine;
use crate::ui::{
    Aside, ChooserRow, Lead, chooser_foot, chooser_group, chooser_row, hairline, hint, icon, theme,
};

/// The user's machines, each with its maker, the file it is sliced into and whether it
/// answers on the network; then the ways to add one and into Machine and resin.
pub fn printer_menu(ui: &mut egui::Ui, window: &mut Window) {
    let chosen = window.machine.slicing.printer_id.clone();
    let mut picked: Option<(String, PrinterProfile)> = None;
    chooser_group(ui, &tr!("picker-your-machines"));
    let mut listed = false;
    for entry in installed_printers(&window.machine.slicing.catalogue) {
        listed = true;
        let profile = &entry.profile;
        let detail = match SlicedFormat::choices_for(profile).next() {
            Some(format) => tr!(
                "picker-machine-detail",
                maker = profile.manufacturer.clone(),
                extension = format.extension()
            ),
            None => profile.manufacturer.clone(),
        };
        let row = ChooserRow {
            lead: Lead::Glyph(icon::PRINTER),
            name: &profile.name,
            detail: &detail,
            aside: answers(&window.machine.network, profile).map(Aside::Dot),
            chosen: chosen.as_deref() == Some(entry.id.as_str()),
        };
        if chooser_row(ui, &row).clicked() {
            picked = Some((entry.id.clone(), profile.clone()));
        }
    }
    if !listed {
        hint(ui, &tr!("picker-no-machine"));
    }
    hairline(ui);
    if chooser_foot(ui, icon::ADD, &tr!("picker-add-machine")).clicked() {
        ui.close();
        crate::panels::open_machines(window.machine);
        window.machine.settings.library = Some(Library::default());
        return;
    }
    if let Some((id, profile)) = picked {
        ui.close();
        apply_printer(window, profile, Some(id));
    }
}

/// The colour of the dot a machine's row carries: `ok` while a printer it lists answers,
/// `text_low` while none does; no dot for a machine that lists none.
fn answers(network: &Network, profile: &PrinterProfile) -> Option<egui::Color32> {
    let colors = theme::colors();
    if profile.machines.is_empty() {
        return None;
    }
    let answered = profile
        .machines
        .iter()
        .any(|machine| network.reach_of(machine) == Reach::Answered);
    Some(if answered { colors.ok } else { colors.text_low })
}

/// The resins set up on the printer in hand, a printer's resins being its own presets, each
/// with its colour, kind, layer height and exposure there; then the way to the printer's
/// resins in Machine and resin, to add one.
pub fn resin_menu(ui: &mut egui::Ui, machine: &mut Machine) {
    let slicing = &mut machine.slicing;
    let chosen = slicing.resin_id.clone();
    let printer_id = slicing.printer_id.clone();
    let printer_name = slicing.printer.as_ref().map(|printer| printer.name.clone());
    if let Some(name) = &printer_name {
        chooser_group(ui, &tr!("picker-on-machine", name = name.clone()));
    }
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
        let tuned = match printer_id.as_deref() {
            Some(id) => entry.profile.for_printer(id),
            None => entry.profile.clone(),
        };
        let detail = tr!(
            "picker-resin-detail",
            kind = tuned.details.kind.clone(),
            layer = format!("{:.0}", tuned.layer_height_mm * 1000.0)
        );
        let exposure = tr!("value-s", value = format!("{:.2}", tuned.exposure_s));
        let row = ChooserRow {
            lead: Lead::Dot(theme::resin_swatch(tuned.details.color)),
            name: &entry.profile.name,
            detail: &detail,
            aside: Some(Aside::Text(&exposure)),
            chosen: chosen.as_deref() == Some(entry.id.as_str()),
        };
        if chooser_row(ui, &row).clicked() {
            picked = Some((entry.id.clone(), entry.profile.clone()));
        }
    }
    if !listed {
        hint(ui, &tr!("picker-no-resin"));
    }
    hairline(ui);
    let printer = slicing.printer_id.clone();
    if printer.is_some() && chooser_foot(ui, icon::ADD, &tr!("picker-add-resin")).clicked() {
        ui.close();
        machine
            .settings
            .open_machines(&slicing.catalogue, printer.as_deref(), None);
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

/// Follows a saved edit of `profile`, the printer in use: an edit that moves the plate stands
/// everything on it again; any other redraws the plate, the models staying where they stand.
pub fn printer_edited(window: &mut Window, profile: PrinterProfile, id: String) {
    let moves_the_plate = window.machine.slicing.printer.as_ref().is_none_or(|old| {
        old.build_volume != profile.build_volume
            || old.display != profile.display
            || old.output != profile.output
    });
    if moves_the_plate {
        apply_printer(window, profile, Some(id));
        return;
    }
    window.doc.plate = BuildPlate::from_profile(&profile);
    window.machine.network.bind_to(Some(id), Some(&profile));
    window.machine.slicing.printer = Some(profile);
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
