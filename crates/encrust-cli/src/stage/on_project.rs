//! A saved plate shaped by the flags: the objects `--only` names, else every one on the plate,
//! repaired, placed, modelled and finished by the edits a run of models makes (ADR 0458).

use std::path::Path;

use anyhow::{Context, Result, bail};
use core_edit::ObjectId;
use core_engine::project::Project;

use super::document::Staging;
use super::{Shaping, finished, laying, modelled, placed, repaired};
use crate::args::JobArgs;
use crate::pipeline::{Watch, saved_model_report};
use crate::profiles::Chosen;
use crate::recipe;

/// `project`'s plate `plate` with every flag given made as edits of the objects they shape,
/// then spread over the plate where `arrange`.
pub(crate) fn shaped(
    path: &Path,
    mut project: Project,
    plate: u32,
    (job, arrange): (&JobArgs, bool),
    watch: &Watch,
) -> Result<Project> {
    let machine = crate::project::machine(&project.manifest, job)?;
    let printer = Some(&machine.printer);
    let drift_mm = machine.resin.compensation.fit_drift_mm();
    project.manifest.active_plate = plate;
    let only = job.only.iter().copied().map(ObjectId).collect();
    let mut staging = Staging::of_project(project, plate, only);
    let unknown = staging.unknown_only();
    if !unknown.is_empty() {
        let ids: Vec<String> = unknown.iter().map(|id| id.0.to_string()).collect();
        bail!(
            "--only {} names no object of plate {}",
            ids.join(","),
            plate + 1
        );
    }
    let first = staging.targets(0).first().copied();
    let mesh = first.and_then(|id| staging.mesh(id));
    let mesh = mesh.context("the plate holds no model for the flags to shape")?;
    let mut found = vec![(saved_model_report(path, &mesh, printer)?, None)];
    let entries = [(path.to_path_buf(), Shaping::of(job)?)];
    job.hollow.check_flags()?;
    repaired::repaired(&mut staging, &mut found, &entries, watch)?;
    placed::placed(
        &mut staging,
        &mut found,
        &entries,
        (machine.printer_id.as_deref(), printer),
        watch,
    )?;
    repaired::pressed(&mut staging, &entries, watch)?;
    modelled::modelled(&mut staging, &job.model, None, watch)?;
    finished::finished(&mut staging, &entries, (printer, drift_mm), watch)?;
    staging = answered(staging, path, &machine, job, watch)?;
    placed::copied(&mut staging, &entries, watch)?;
    if arrange {
        laying::arranged(&mut staging, &entries, printer, watch)?;
    }
    Ok(staging.into_project())
}

/// `staging` with Check's answers the flags ask for, on the machine the run slices for.
fn answered(
    staging: Staging,
    path: &Path,
    machine: &core_engine::Machine,
    job: &JobArgs,
    watch: &Watch,
) -> Result<Staging> {
    if !job.fixes.asked() {
        return Ok(staging);
    }
    let chosen = Chosen {
        printer: Some(machine.printer.clone()),
        material: machine.resin.clone(),
        invented_exposure: false,
    };
    let layer_height_mm = (job.slicing.layer_height).unwrap_or(machine.resin.layer_height_mm);
    let ctx = recipe::Ctx {
        job,
        chosen: &chosen,
        layer_height_mm,
        watch,
    };
    recipe::answered(staging, path, &ctx)
}
