//! A `.encrust` project saved by the window, sliced a plate at a time. Flags that shape a model
//! are refused; printer and resin are named by id only (ADR 0261) and flags may change them,
//! and the print settings are theirs under what the flags set (ADR 0380).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use core_engine::project::{Manifest, Project, load};
use core_engine::{Machine, Opening, open_plate};
use core_format::ExposurePlan;
use printer_profiles::{MaterialProfile, PrinterProfile};

use crate::args::JobArgs;
use crate::pipeline::{cutting_of, overrides_of, raster_window, warn_if_grey_ignored};
use crate::profiles::catalogue;
use crate::sliced_file::now_unix_s;
use crate::stage::{Part, Staged};

/// Opens a project to stage its plates from.
pub fn open(path: &Path) -> Result<Project> {
    load(path).with_context(|| format!("cannot open the project {}", path.display()))
}

/// The plates holding a model to print, by index and name, as the window slices them all.
pub fn printable_plates(manifest: &Manifest) -> Vec<(u32, &str)> {
    (0..manifest.plates.len() as u32)
        .filter(|&plate| {
            manifest
                .objects
                .iter()
                .any(|object| object.printable_on(plate))
        })
        .map(|plate| (plate, manifest.plates[plate as usize].as_str()))
        .collect()
}

/// The plate `--plate` names, counted from one as the window's plate bar shows them.
pub fn chosen_plate(manifest: &Manifest, number: u32) -> Result<u32> {
    let count = manifest.plates.len() as u32;
    if !(1..=count).contains(&number) {
        bail!("--plate {number} is not a plate of this project, which has {count}");
    }
    let plate = number - 1;
    if !printable_plates(manifest)
        .iter()
        .any(|(index, _)| *index == plate)
    {
        bail!("plate {number} holds nothing to print");
    }
    Ok(plate)
}

/// The plate the project was left on, said aloud when other plates hold models too.
fn saved_plate(path: &Path, manifest: &Manifest) -> u32 {
    let active = manifest.active_plate;
    let others = printable_plates(manifest)
        .iter()
        .filter(|(plate, _)| *plate != active)
        .count();
    if others > 0 {
        let name = manifest
            .plates
            .get(active as usize)
            .map_or("", String::as_str);
        tracing::warn!(
            "{} has {others} more plate(s) with models; slicing plate {} \"{name}\", the one it \
             was saved on. --plate N or --all-plates slices the others",
            path.display(),
            active + 1
        );
    }
    active
}

/// Stages `plate` of the project, or the plate it was left on.
pub fn stage(path: &Path, project: &Project, plate: Option<u32>, job: &JobArgs) -> Result<Staged> {
    refuse_shaping_flags(job)?;
    let manifest = &project.manifest;
    let plate = plate.unwrap_or_else(|| saved_plate(path, manifest));
    let machine = machine(manifest, job)?;
    let built = open_plate(
        project,
        &machine,
        &Opening {
            plate,
            raster_window: raster_window(&job.raster),
            created_unix_s: now_unix_s(),
        },
    )
    .with_context(|| format!("cannot build the plate {} holds", path.display()))?;
    warn_if_grey_ignored(&job.raster, Some(&built.printer));
    // The project keeps no print settings: the resin's, under what the flags change (ADR 0380).
    let material = match job.slicing.layer_height {
        Some(height_mm) => built.material.rescaled_to(height_mm),
        None => built.material,
    };

    let on_plate: Vec<_> = manifest
        .objects
        .iter()
        .filter(|object| object.printable_on(plate))
        .collect();
    let drainage = job.slicing.check_drainage
        || on_plate.iter().any(|object| {
            let hollow = &object.hollow;
            hollow.built.is_some() || !hollow.drains.is_empty() || !hollow.channels.is_empty()
        });
    Ok(Staged {
        parts: on_plate
            .iter()
            .map(|object| Part {
                input: PathBuf::from(&object.name),
                import: None,
                oriented: None,
                hollow: None,
                supports: None,
            })
            .collect(),
        models: built.models,
        cutting: cutting_of(job, material.layer_height_mm, &material.compensation, false),
        panel: overrides_of(&job.raster, material.edges),
        exposure: ExposurePlan::new(job.slicing.exposure_at.clone()),
        printer: Some(built.printer),
        material,
        remove_islands: built.remove_islands,
        hanging_passes: built.hanging_passes,
        drainage,
    })
}

/// The printer and resin the flags name, or else the ones the project names by id, as
/// the user's catalogue has them; the engine tunes the resin to the machine as the window does.
fn machine(manifest: &Manifest, job: &JobArgs) -> Result<Machine> {
    let flags = &job.profile;
    let (printer, printer_id) = match (&flags.profile, &flags.printer) {
        (Some(path), _) => {
            let printer = PrinterProfile::load(path)
                .with_context(|| format!("cannot load {}", path.display()))?;
            (printer, None)
        }
        (None, Some(id)) => {
            let why = format!("--printer {id} is not among your printers");
            (printer_of(id, &why)?, Some(id.clone()))
        }
        (None, None) => {
            let id = manifest
                .printer
                .as_deref()
                .context("the project names no printer: pick one with --printer")?;
            let why =
                format!("the project's printer {id} is not among yours: pick one with --printer");
            (printer_of(id, &why)?, Some(id.to_owned()))
        }
    };
    let resin = match (&flags.material, &flags.resin) {
        (Some(path), _) => MaterialProfile::load(path)
            .with_context(|| format!("cannot load {}", path.display()))?,
        (None, Some(id)) => resin_of(id, &format!("--resin {id} is not among your resins"))?,
        (None, None) => {
            let id = manifest
                .resin
                .as_deref()
                .context("the project names no resin: pick one with --resin")?;
            let why = format!("the project's resin {id} is not among yours: pick one with --resin");
            resin_of(id, &why)?
        }
    };
    Ok(Machine {
        printer,
        printer_id,
        resin,
    })
}

fn printer_of(id: &str, why: &str) -> Result<PrinterProfile> {
    let catalogue = catalogue()?;
    let entry = catalogue.printer(id).with_context(|| {
        format!("{why}; run `encrust profiles list` to see the printers there are")
    })?;
    Ok(entry.profile.clone())
}

fn resin_of(id: &str, why: &str) -> Result<MaterialProfile> {
    let catalogue = catalogue()?;
    let entry = catalogue.resin(id).with_context(|| {
        format!("{why}; run `encrust profiles list` to see the resins there are")
    })?;
    Ok(entry.profile.clone())
}

/// The flags a project already answers for itself.
fn refuse_shaping_flags(job: &JobArgs) -> Result<()> {
    job.hollow.check_flags()?;
    let import = &job.import;
    let transform = &import.transform;
    let given = [
        ("--rotate", transform.rotate.is_some()),
        ("--scale", transform.scale.is_some()),
        ("--center", transform.center),
        ("--orient", transform.orient),
        ("--relief", import.relief.is_some()),
        ("--hollow", job.hollow.wanted()),
        ("--drain-at or --channel", job.hollow.cutting()),
        (
            "--supports",
            job.supports.supports.is_some() || job.supports.support_profile.is_some(),
        ),
        ("--support-cavity", job.supports.support_cavity),
        ("--remove-islands", job.raster.remove_islands),
        ("--hanging-passes", job.raster.hanging_passes != 0),
    ];
    if let Some((flag, _)) = given.iter().find(|(_, given)| *given) {
        bail!("{flag} does not apply to a project, which carries its own; change it in the window");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use core_engine::project::save;

    use super::*;

    #[derive(Parser)]
    struct Flags {
        #[command(flatten)]
        job: JobArgs,
    }

    #[test]
    fn the_hanging_passes_a_project_was_saved_with_are_kept() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cube.encrust");
        let mut project = load(&fixture).expect("the fixture is a project");
        project.manifest.slicing.remove_islands = true;
        project.manifest.slicing.hanging_passes = 2;
        let path = std::env::temp_dir().join("encrust-cli-hanging-passes.encrust");
        save(&path, &project).expect("the temp dir is writable");

        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let flags = Flags::parse_from([
            "encrust".into(),
            "--profile".into(),
            fixtures.join("project-panel.toml"),
            "--material".into(),
            fixtures.join("test-resin.toml"),
        ]);
        let project = open(&path).expect("the project was just saved");
        let staged = stage(&path, &project, None, &flags.job).expect("it stages");
        assert!(staged.remove_islands);
        assert_eq!(staged.hanging_passes, 2, "as the window would slice it");
    }
}
