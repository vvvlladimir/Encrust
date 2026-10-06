//! A `.encrust` project saved by the window, sliced without one.
//!
//! The project carries its own models, placements, supports and walls, so the flags that
//! shape a model are refused rather than quietly ignored. The printer, the resin, the
//! layer height and how many layers are cut at once are what a flag may still change.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use core_engine::project::{Chosen, Project, load};
use core_engine::{Opening, open_plate};
use printer_profiles::{Catalogue, MaterialProfile};

use crate::args::JobArgs;
use crate::pipeline::raster_window;
use crate::profiles::{self, Selection};
use crate::sliced_file::now_unix_s;
use crate::stage::{Part, Staged};

/// Stages the plate the project was left on.
pub fn stage(path: &Path, job: &JobArgs) -> Result<Staged> {
    refuse_shaping_flags(job)?;
    let mut project =
        load(path).with_context(|| format!("cannot open the project {}", path.display()))?;
    take_overrides(&mut project, job)?;

    let manifest = &project.manifest;
    let plate = open_plate(
        &project,
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
        drainage,
    })
}

/// Puts the printer, resin and layer height the flags name in place of the project's, so
/// the engine tunes the resin to the machine the same way the window does.
fn take_overrides(project: &mut Project, job: &JobArgs) -> Result<()> {
    let flags = &job.profile;
    let manifest = &mut project.manifest;
    if flags.profile.is_some() || flags.printer.is_some() {
        let chosen = profiles::resolve(&Selection {
            printer_id: flags.printer.as_deref(),
            printer_path: flags.profile.as_deref(),
            resin_id: None,
            resin_path: None,
        })?;
        manifest.printer = chosen.printer.map(|profile| Chosen {
            id: flags
                .profile
                .is_none()
                .then(|| flags.printer.clone())
                .flatten(),
            profile,
        });
    }
    if let Some(path) = &flags.material {
        let profile = MaterialProfile::load(path)
            .with_context(|| format!("cannot load {}", path.display()))?;
        manifest.resin = Some(Chosen { id: None, profile });
    } else if let Some(id) = &flags.resin {
        let catalogue = Catalogue::load().context("cannot read the profile catalogue")?;
        let entry = catalogue
            .resin(id)
            .context("run `encrust profiles list` to see the resins there are")?;
        manifest.resin = Some(Chosen {
            id: Some(id.clone()),
            profile: entry.profile.clone(),
        });
    }
    if let Some(height_mm) = job.slicing.layer_height {
        manifest.slicing.layer_height_mm = height_mm;
    }
    Ok(())
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
        ("--adaptive", job.slicing.adaptive),
        (
            "--samples-per-layer",
            job.slicing.samples_per_layer.get() != 1,
        ),
        ("--exposure-at", !job.slicing.exposure_at.is_empty()),
        ("--remove-islands", job.raster.remove_islands),
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
