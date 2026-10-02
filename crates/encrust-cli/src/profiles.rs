//! Resolving `--printer` and `--resin` against the catalogue, and listing what is in it.

use std::path::Path;

use anyhow::{Context, Result};
use printer_profiles::{Catalogue, MaterialProfile, PrinterProfile};

/// What the four profile arguments name, before any of it is loaded.
pub struct Selection<'a> {
    pub printer_id: Option<&'a str>,
    pub printer_path: Option<&'a Path>,
    pub resin_id: Option<&'a str>,
    pub resin_path: Option<&'a Path>,
}

/// The printer to slice for and the resin to expose with, already tuned for it.
pub struct Chosen {
    pub printer: Option<PrinterProfile>,
    pub material: MaterialProfile,
}

/// Resolves both profiles. A path always wins over a catalogue id, so a profile being
/// tuned by hand is reachable without taking it out of the catalogue first.
pub fn resolve(selection: &Selection) -> Result<Chosen> {
    let catalogue = (selection.printer_id.is_some() || selection.resin_id.is_some())
        .then(Catalogue::load)
        .transpose()
        .context("cannot read the profile catalogue")?;

    let printer = match (selection.printer_path, selection.printer_id) {
        (Some(path), _) => Some(
            PrinterProfile::load(path)
                .with_context(|| format!("cannot load {}", path.display()))?,
        ),
        (None, Some(id)) => Some(printer_from(catalogue.as_ref(), id)?),
        (None, None) => None,
    };

    let printer_id = selection
        .printer_path
        .is_none()
        .then_some(selection.printer_id)
        .flatten();
    let material = resolve_material(catalogue.as_ref(), selection, printer_id)?;
    Ok(Chosen { printer, material })
}

fn printer_from(catalogue: Option<&Catalogue>, id: &str) -> Result<PrinterProfile> {
    let catalogue = catalogue.context("the catalogue was not loaded")?;
    let entry = catalogue.printer(id).with_context(|| {
        format!(
            "run --list-profiles to see the {} printers there are",
            catalogue.printers().count()
        )
    })?;
    Ok(entry.profile.clone())
}

/// The resin, retuned for the chosen printer. Without a printer id there is nothing to
/// tune against, so the resin's own numbers are used and the user is told.
fn resolve_material(
    catalogue: Option<&Catalogue>,
    selection: &Selection,
    printer_id: Option<&str>,
) -> Result<MaterialProfile> {
    if let Some(path) = selection.resin_path {
        let resin = MaterialProfile::load(path)
            .with_context(|| format!("cannot load {}", path.display()))?;
        return Ok(match printer_id {
            Some(id) => resin.starting_point(id),
            None => resin,
        });
    }

    let Some(catalogue) = catalogue else {
        return Ok(MaterialProfile::default());
    };
    let entry = match selection.resin_id {
        Some(id) => catalogue.resin(id).with_context(|| {
            format!(
                "run --list-profiles to see the {} resins there are",
                catalogue.resins().count()
            )
        })?,
        // A printer alone still needs a resin: the one the catalogue measured for it.
        None => match printer_id.and_then(|id| catalogue.default_resin_for(id)) {
            Some(entry) => entry,
            None => return Ok(MaterialProfile::default()),
        },
    };

    let Some(printer_id) = printer_id else {
        tracing::warn!(
            "{} was not tuned for any printer here; its own numbers are a starting point",
            entry.id
        );
        return Ok(entry.profile.clone());
    };
    if !entry.profile.is_tuned_for(printer_id) {
        tracing::warn!(
            "{} carries no numbers for {printer_id}; its own are a starting point, not a calibration",
            entry.id
        );
    }
    Ok(entry.profile.starting_point(printer_id))
}

/// Prints the catalogue, one line per profile, marking what came from the user.
pub fn list() -> Result<()> {
    let catalogue = Catalogue::load().context("cannot read the profile catalogue")?;

    println!("Printers:");
    for entry in catalogue.printers() {
        let printer = &entry.profile;
        let volume = &printer.build_volume;
        println!(
            "  {:<24} {} {} — {:.0} x {:.0} x {:.0} mm, {} x {} px{}",
            entry.id,
            printer.manufacturer,
            printer.name,
            volume.x,
            volume.y,
            volume.z,
            printer.display.width_px,
            printer.display.height_px,
            origin(&entry.source),
        );
    }

    println!("\nResins:");
    for entry in catalogue.resins() {
        let tuned = catalogue
            .printers()
            .filter(|printer| entry.profile.is_tuned_for(&printer.id))
            .count();
        println!(
            "  {:<24} {} — {:.2} s at {:.3} mm, tuned for {tuned} of {} printers{}",
            entry.id,
            entry.profile.name,
            entry.profile.exposure_s,
            entry.profile.layer_height_mm,
            catalogue.printers().count(),
            origin(&entry.source),
        );
    }
    Ok(())
}

fn origin(source: &printer_profiles::Source) -> &'static str {
    match source {
        printer_profiles::Source::Bundled => "",
        printer_profiles::Source::User(_) => "  [user]",
    }
}
