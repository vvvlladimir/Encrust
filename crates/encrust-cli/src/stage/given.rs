//! What the inputs on the command line are, and the flags refused for them before any run.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use printer_profiles::PrinterProfile;

use super::Shaping;
use crate::args::JobArgs;
use crate::project;

/// What the inputs on the command line are: models, one file describing a plate, or several
/// projects laid on one plate.
pub(super) enum Given<'a> {
    Models(&'a [PathBuf]),
    PlateFile(&'a Path),
    Project(&'a Path),
    Projects(&'a [PathBuf]),
}

pub(super) fn given(inputs: &[PathBuf]) -> Result<Given<'_>> {
    let plate = inputs.iter().find(|path| describes_a_plate(path));
    match (plate, inputs) {
        (None, []) => bail!("nothing to slice: name a model, a plate file or a project"),
        (None, _) => Ok(Given::Models(inputs)),
        (Some(_), [only]) if extension_of(only).as_deref() == Some("toml") => {
            Ok(Given::PlateFile(only))
        }
        (Some(_), [only]) => Ok(Given::Project(only)),
        (Some(_), _) if inputs.iter().all(|path| describes_a_project(path)) => {
            Ok(Given::Projects(inputs))
        }
        (Some(path), _) => bail!(
            "{} describes a whole plate, so it is sliced on its own",
            path.display()
        ),
    }
}

/// Whether `path` names a project saved by the window.
pub fn describes_a_project(path: &Path) -> bool {
    extension_of(path).as_deref() == Some("encrust")
}

/// Whether `path` names a whole plate, a plate file or a project, rather than a model.
pub fn describes_a_plate(path: &Path) -> bool {
    matches!(extension_of(path).as_deref(), Some("toml" | "encrust"))
}

fn extension_of(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
}

/// The flags of a plate made here, which a project saved by the window answers for itself.
pub fn refuse_for_a_project(arrange: bool, job: &JobArgs) -> Result<()> {
    project::refuse_shaping_flags(job)?;
    if job.model.wanted() {
        bail!("a project's models are combined, cut and split in the window");
    }
    if arrange {
        bail!("a project stands where it was saved; arrange it in the window");
    }
    Ok(())
}

/// Flags that point at one spot on one model mean nothing across several; models `combined`
/// make one, which `--center` stands in the middle.
pub fn refuse_one_model_flags(shaping: &Shaping, combined: bool) -> Result<()> {
    if shaping.import.transform.center && !combined {
        bail!("--center stands every model in the middle of the plate; --arrange spreads them");
    }
    if shaping.hollow.pointing() {
        bail!("--drain-at and --channel point at one model; a plate file places them per model");
    }
    Ok(())
}

/// Refuses `--support-cavity` where it would hold nothing up: no supports, or no cavity.
pub(super) fn refuse_idle_cavity(job: &JobArgs, shaping: &Shaping) -> Result<()> {
    if !job.supports.support_cavity || job.recipe_asked().is_some() {
        return Ok(());
    }
    if shaping.supports.is_none() {
        bail!("--support-cavity stands supports in the cavity, and --supports was not given");
    }
    if !shaping.hollow.wanted() {
        bail!("--support-cavity holds up the cavity --hollow leaves, and --hollow was not given");
    }
    Ok(())
}

/// Refuses an adaptive run on a machine that steps the plate by the header's layer
/// height, before the stack is cut rather than when the file comes to be written.
pub(super) fn check_adaptive(job: &JobArgs, printer: Option<&PrinterProfile>) -> Result<()> {
    let Some(printer) = printer.filter(|_| job.slicing.adaptive) else {
        return Ok(());
    };
    if printer.firmware.variable_layer_height {
        return Ok(());
    }
    bail!(
        "{} steps the plate by the header's layer height, so a stack of layers of \
         different thicknesses cannot be printed on it; --adaptive needs a profile whose \
         [firmware] claims variable_layer_height",
        printer.name
    )
}
