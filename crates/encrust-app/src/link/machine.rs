//! The printer and resin a revision names, set on the window as its menus set them, so the
//! plate is drawn on the job's machine.

use std::path::Path;

use anyhow::{Context, Result};
use printer_profiles::{MaterialProfile, PrinterProfile};

use crate::panels::Window;

/// Puts the window on `printer` and `resin`, catalogue ids or TOML paths, where it is on
/// another.
pub fn set_machine(window: &mut Window, (printer, resin): (&str, &str)) -> Result<()> {
    if !printer.is_empty() && window.machine.slicing.printer_id.as_deref() != Some(printer) {
        let (profile, id) = if is_toml(printer) {
            let loaded = PrinterProfile::load(Path::new(printer))
                .with_context(|| format!("reading {printer}"))?;
            (loaded, None)
        } else {
            let entry = window.machine.slicing.catalogue.printer(printer)?;
            (entry.profile.clone(), Some(printer.to_owned()))
        };
        crate::profiles::apply_printer(window, profile, id);
    }
    let held = &mut window.machine.slicing;
    if !resin.is_empty() && held.resin_id.as_deref() != Some(resin) {
        let (profile, id) = if is_toml(resin) {
            let loaded = MaterialProfile::load(Path::new(resin))
                .with_context(|| format!("reading {resin}"))?;
            (loaded, None)
        } else {
            let entry = held.catalogue.resin(resin)?;
            (entry.profile.clone(), Some(resin.to_owned()))
        };
        held.resin_id = id;
        held.set_material(profile);
    }
    Ok(())
}

fn is_toml(named: &str) -> bool {
    Path::new(named)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("toml"))
}
