//! Resolving `--printer` and `--resin` against the catalogue, and listing what is in it.

use std::fmt;
use std::path::Path;

use anyhow::{Context, Result};
use core_engine::project::{DrainState, HollowState};
use printer_profiles::{Catalogue, Kind, MaterialProfile, PrinterProfile, SupportProfile};
use serde::Serialize;

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
    /// Whether the exposure in `material` is one nobody measured: a machine taken out of
    /// the catalogue with no resin named for it. Nothing ships a resin (ADR 0196), so
    /// there is nothing to fall back on; see [`Chosen::measured`].
    pub invented_exposure: bool,
}

impl Chosen {
    /// Refuses a job that would print at an exposure nobody measured.
    pub fn measured(&self) -> Result<()> {
        if !self.invented_exposure {
            return Ok(());
        }
        anyhow::bail!(
            "no resin to print with: pass --resin <id> or --material <file.toml>. An \
             exposure is measured on your own machine, not shipped, so none is assumed \
             here; `encrust profiles list` shows the resins you have"
        )
    }
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
    let named_a_resin = selection.resin_path.is_some() || selection.resin_id.is_some();
    Ok(Chosen {
        printer,
        material,
        invented_exposure: printer_id.is_some() && !named_a_resin,
    })
}

fn printer_from(catalogue: Option<&Catalogue>, id: &str) -> Result<PrinterProfile> {
    let catalogue = catalogue.context("the catalogue was not loaded")?;
    let entry = catalogue.printer(id).with_context(|| {
        format!(
            "run `encrust profiles list` to see the {} printers there are",
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
    let Some(id) = selection.resin_id else {
        // Nothing is shipped to fall back on. The stock numbers stand so that a report
        // over geometry still runs; a job that exposes resin is refused in `Chosen`.
        return Ok(MaterialProfile::default());
    };
    let entry = catalogue.resin(id).with_context(|| {
        format!(
            "run `encrust profiles list` to see the {} resins there are",
            catalogue.resins().count()
        )
    })?;

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

/// The support profile `id` names in the catalogue, shipped or the user's own.
pub fn support(id: &str) -> Result<SupportProfile> {
    let catalogue = Catalogue::load().context("cannot read the profile catalogue")?;
    let entry = catalogue.support(id).with_context(|| {
        format!(
            "run `encrust profiles list` to see the {} support profiles there are",
            catalogue.supports().count()
        )
    })?;
    Ok(entry.profile.clone())
}

/// Every kind of profile the catalogue keeps, in the order they are listed.
const KINDS: [Kind; 5] = [
    Kind::Printer,
    Kind::Resin,
    Kind::Support,
    Kind::Hollow,
    Kind::Drain,
];

/// One profile of the catalogue as the TOML it is kept in, and which kind it turned out to
/// be. An id two kinds share needs `kind` to say which.
pub fn show(id: &str, kind: Option<Kind>) -> Result<(Kind, String)> {
    let catalogue = Catalogue::load().context("cannot read the profile catalogue")?;
    let named = Path::new(id);
    let found: Vec<(Kind, String)> = KINDS
        .into_iter()
        .filter(|candidate| kind.is_none_or(|wanted| wanted == *candidate))
        .filter_map(|candidate| {
            let toml = match candidate {
                Kind::Printer => catalogue.printer(id).ok()?.profile.to_toml_string(named),
                Kind::Resin => catalogue.resin(id).ok()?.profile.to_toml_string(named),
                Kind::Support => catalogue.support(id).ok()?.profile.to_toml_string(named),
                Kind::Hollow | Kind::Drain => catalogue
                    .presets(candidate)
                    .find(|entry| entry.id == id)?
                    .profile
                    .to_toml_string(named),
            };
            Some(toml.map(|toml| (candidate, toml)))
        })
        .collect::<Result<_, _>>()
        .with_context(|| format!("cannot write {id} out as TOML"))?;
    let mut found = found.into_iter();
    match (found.next(), found.next()) {
        (Some(one), None) => Ok(one),
        (None, _) => anyhow::bail!("no profile is called {id}; `encrust profiles list` says which"),
        (Some(_), Some(_)) => anyhow::bail!("{id} names more than one kind; pick one with --kind"),
    }
}

/// Takes the profile file at `path` into the user's catalogue as `kind`, under an id of
/// its own; a resin is set up on `printer`. Answers the new id.
pub fn import(path: &Path, kind: Kind, printer: Option<&str>) -> Result<String> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    if printer.is_some() && kind != Kind::Resin {
        anyhow::bail!(
            "--printer sets a resin up on a machine; a {} has none",
            kind.label()
        );
    }
    let mut catalogue = Catalogue::load().context("cannot read the profile catalogue")?;
    if let Some(id) = printer {
        catalogue
            .printer(id)
            .context("run `encrust profiles list` to see the printers there are")?;
    }
    let imported = match kind {
        Kind::Hollow => catalogue.import_preset::<HollowState>(kind, &text, path),
        Kind::Drain => catalogue.import_preset::<DrainState>(kind, &text, path),
        Kind::Printer | Kind::Resin | Kind::Support => catalogue.import(kind, &text, path, printer),
    };
    imported.with_context(|| format!("cannot import {} as a {}", path.display(), kind.label()))
}

/// The catalogue as `profiles list` prints it.
#[derive(Serialize)]
pub struct Listing {
    printers: Vec<PrinterLine>,
    resins: Vec<ResinLine>,
    supports: Vec<SupportLine>,
    hollow: Vec<PresetLine>,
    drain: Vec<PresetLine>,
}

#[derive(Serialize)]
struct PrinterLine {
    id: String,
    manufacturer: String,
    name: String,
    build_volume_mm: [f32; 3],
    width_px: u32,
    height_px: u32,
    user: bool,
}

#[derive(Serialize)]
struct ResinLine {
    id: String,
    name: String,
    exposure_s: f32,
    layer_height_mm: f32,
    /// Printers in the catalogue this resin carries measured numbers for.
    tuned_for: usize,
    user: bool,
}

#[derive(Serialize)]
struct SupportLine {
    id: String,
    name: String,
    /// How far the lowest point of a part stands off the plate for this profile, mm.
    z_lift_mm: f32,
    max_overhang_deg: f32,
    density: f32,
    user: bool,
}

/// Named settings of a tool, which are always the user's own.
#[derive(Serialize)]
struct PresetLine {
    id: String,
    name: String,
}

/// Reads the catalogue, marking what came from the user.
pub fn list() -> Result<Listing> {
    let catalogue = Catalogue::load().context("cannot read the profile catalogue")?;
    let printers = catalogue
        .printers()
        .map(|entry| {
            let printer = &entry.profile;
            let volume = &printer.build_volume;
            PrinterLine {
                id: entry.id.clone(),
                manufacturer: printer.manufacturer.clone(),
                name: printer.name.clone(),
                build_volume_mm: [volume.x, volume.y, volume.z],
                width_px: printer.display.width_px,
                height_px: printer.display.height_px,
                user: is_user(&entry.source),
            }
        })
        .collect();
    let resins = catalogue
        .resins()
        .map(|entry| ResinLine {
            id: entry.id.clone(),
            name: entry.profile.name.clone(),
            exposure_s: entry.profile.exposure_s,
            layer_height_mm: entry.profile.layer_height_mm,
            tuned_for: catalogue
                .printers()
                .filter(|printer| entry.profile.is_tuned_for(&printer.id))
                .count(),
            user: is_user(&entry.source),
        })
        .collect();
    let supports = catalogue
        .supports()
        .map(|entry| SupportLine {
            id: entry.id.clone(),
            name: entry.profile.name.clone(),
            z_lift_mm: entry.profile.z_lift_mm,
            max_overhang_deg: entry.profile.max_overhang_deg,
            density: entry.profile.density,
            user: is_user(&entry.source),
        })
        .collect();
    for path in catalogue.unreadable() {
        tracing::warn!("{} is not a preset and was left out", path.display());
    }
    let presets = |kind| {
        catalogue
            .presets(kind)
            .map(|entry| PresetLine {
                id: entry.id.clone(),
                name: entry.profile.name.clone(),
            })
            .collect()
    };
    Ok(Listing {
        printers,
        resins,
        supports,
        hollow: presets(Kind::Hollow),
        drain: presets(Kind::Drain),
    })
}

impl fmt::Display for Listing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Printers:")?;
        for printer in &self.printers {
            let [x, y, z] = printer.build_volume_mm;
            writeln!(
                f,
                "  {:<24} {} {} — {x:.0} x {y:.0} x {z:.0} mm, {} x {} px{}",
                printer.id,
                printer.manufacturer,
                printer.name,
                printer.width_px,
                printer.height_px,
                origin(printer.user),
            )?;
        }

        writeln!(f, "\nResins:")?;
        for resin in &self.resins {
            writeln!(
                f,
                "  {:<24} {} — {:.2} s at {:.3} mm, tuned for {} of {} printers{}",
                resin.id,
                resin.name,
                resin.exposure_s,
                resin.layer_height_mm,
                resin.tuned_for,
                self.printers.len(),
                origin(resin.user),
            )?;
        }

        writeln!(f, "\nSupports:")?;
        for support in &self.supports {
            writeln!(
                f,
                "  {:<24} {} — {:.1} mm lift, {:.0} deg overhang, density {:.1}{}",
                support.id,
                support.name,
                support.z_lift_mm,
                support.max_overhang_deg,
                support.density,
                origin(support.user),
            )?;
        }

        for (title, presets) in [("Hollow", &self.hollow), ("Drain", &self.drain)] {
            if presets.is_empty() {
                continue;
            }
            writeln!(f, "\n{title} presets:")?;
            for preset in presets {
                writeln!(f, "  {:<24} {}", preset.id, preset.name)?;
            }
        }
        Ok(())
    }
}

fn is_user(source: &printer_profiles::Source) -> bool {
    matches!(source, printer_profiles::Source::User(_))
}

fn origin(user: bool) -> &'static str {
    if user { "  [user]" } else { "" }
}
