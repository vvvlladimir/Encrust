//! A `.encrust` project saved by the window, sliced without one. Flags that shape a model are
//! refused; printer and resin are named by id only (ADR 0261) and flags may still change them.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use core_engine::project::{Manifest, load};
use core_engine::{Machine, Opening, open_plate};
use printer_profiles::{MaterialProfile, PrinterProfile};

use crate::args::JobArgs;
use crate::pipeline::raster_window;
use crate::profiles::catalogue;
use crate::sliced_file::now_unix_s;
use crate::stage::{Part, Staged};

/// Stages the plate the project was left on.
pub fn stage(path: &Path, job: &JobArgs) -> Result<Staged> {
    refuse_shaping_flags(job)?;
    let mut project =
        load(path).with_context(|| format!("cannot open the project {}", path.display()))?;
    if let Some(height_mm) = job.slicing.layer_height {
        project.manifest.slicing.layer_height_mm = height_mm;
    }
    let machine = machine(&project.manifest, job)?;

    let manifest = &project.manifest;
    let plate = open_plate(
        &project,
        &machine,
        &Opening {
            plate: manifest.active_plate,
            raster_window: raster_window(&job.raster),
            created_unix_s: now_unix_s(),
        },
    )
    .with_context(|| format!("cannot build the plate {} holds", path.display()))?;

    let on_plate: Vec<_> = manifest
        .objects
        .iter()
        .filter(|object| object.plate == manifest.active_plate && object.visible)
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
        models: plate.models,
        printer: Some(plate.printer),
        material: plate.material,
        // How many layers are cut at once trades memory for time and shapes nothing.
        cutting: core_engine::Cutting {
            slice_window: job.slicing.slice_window,
            ..plate.cutting
        },
        panel: plate.panel,
        exposure: plate.exposure,
        remove_islands: plate.remove_islands,
        hanging_passes: plate.hanging_passes,
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
        ("--adaptive", job.slicing.adaptive),
        (
            "--samples-per-layer",
            job.slicing.samples_per_layer.get() != 1,
        ),
        ("--exposure-at", !job.slicing.exposure_at.is_empty()),
        ("--remove-islands", job.raster.remove_islands),
        ("--hanging-passes", job.raster.hanging_passes != 0),
        ("--no-anti-alias", job.raster.no_anti_alias),
        ("--grey-levels", job.raster.grey_levels.is_some()),
        ("--grey-floor", job.raster.grey_floor.is_some()),
        ("--blur", job.raster.blur != 0),
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
        let staged = stage(&path, &flags.job).expect("it stages");
        assert!(staged.remove_islands);
        assert_eq!(staged.hanging_passes, 2, "as the window would slice it");
    }
}
