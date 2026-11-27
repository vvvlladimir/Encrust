//! A `.encrust` project saved by the window, sliced a plate at a time, its objects shaped by the
//! flags (ADR 0458); its printer and resin by id (ADR 0261), its settings under the flags' (0380).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use core_document::{Document, apply};
use core_edit::Edit;
use core_engine::project::{Manifest, ModelMeshes, ObjectState, Project, insert, load};
use core_geometry::placed_aabb;
use std::time::Duration;

use crate::hollowing::HollowReport;
use crate::supports::SupportReport;
use core_engine::{Machine, Opening, open_plate};
use core_format::ExposurePlan;
use core_volume::Cancel;
use printer_profiles::{MaterialProfile, PrinterProfile};

use crate::args::JobArgs;
use crate::meeting;
use crate::pipeline::{Watch, cutting_of, overrides_of, raster_window, warn_if_grey_ignored};
use crate::profiles::catalogue;
use crate::report::FitCheck;
use crate::sliced_file::now_unix_s;
use crate::stage::{Part, Staged, bounds, print_off_the_plate};

/// Opens a project to stage its plates from.
pub fn open(path: &Path) -> Result<Project> {
    load(path).with_context(|| format!("cannot open the project {}", path.display()))
}

/// The plate each of `paths` was left on, laid on one in the order given, the first's settings
/// kept; spread, where `arranged`, over the printer its flags or the first project name.
pub fn laid_together(paths: &[PathBuf], arranged: Option<&JobArgs>) -> Result<Project> {
    let mut opened = paths.iter().map(|path| open(path));
    let first = opened.next().context("no project to lay on the plate")??;
    let mut laid = Project {
        manifest: Manifest {
            plates: vec![first.manifest.plates[first.manifest.active_plate as usize].clone()],
            active_plate: 0,
            objects: Vec::new(),
            ..first.manifest.clone()
        },
        models: Vec::new(),
        preview: None,
    };
    insert(&mut laid, first, 0);
    for project in opened {
        insert(&mut laid, project?, 0);
    }
    let Some(job) = arranged else {
        return Ok(laid);
    };
    let printer = machine(&laid.manifest, job)?.printer;
    let (low_mm, plate_mm) = crate::stage::usable_area(&printer);
    let spread = Edit::Arrange {
        low_mm,
        plate_mm,
        clearance_mm: core_edit::defaults::arrange_clearance_mm(),
    };
    let mut document = Document::from_project(laid);
    apply(&mut document, &spread, Cancel::never()).context("arranging the projects")?;
    Ok(document.into_project())
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

/// `plate` of `project`, counted from one, staged, with what stands off the plate said.
pub fn stage_checked(
    path: &Path,
    project: &Project,
    plate: Option<u32>,
    job: &JobArgs,
    watch: &Watch,
) -> Result<Staged> {
    let plate = plate
        .map(|number| chosen_plate(&project.manifest, number))
        .transpose()?;
    let mut staged = stage(path, project, plate, job)?;
    if watch.talk {
        print_off_the_plate(&staged.parts);
    }
    let owners: Vec<usize> = (0..staged.models.len()).collect();
    meeting::mark(&mut staged.parts, &staged.models, &owners, watch.talk);
    Ok(staged)
}

/// Stages `plate` of the project, or the plate it was left on.
pub fn stage(path: &Path, project: &Project, plate: Option<u32>, job: &JobArgs) -> Result<Staged> {
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

    let on_plate: Vec<_> = (manifest.objects.iter().zip(&project.models))
        .filter(|(object, _)| object.printable_on(plate))
        .collect();
    let drainage = job.slicing.check_drainage
        || on_plate.iter().any(|(object, _)| {
            let hollow = &object.hollow;
            hollow.built.is_some() || !hollow.drains.is_empty() || !hollow.channels.is_empty()
        });
    Ok(Staged {
        model_ids: on_plate.iter().map(|(object, _)| object.id).collect(),
        parts: on_plate
            .iter()
            .zip(&built.models)
            .map(|((object, meshes), model)| Part {
                id: Some(object.id),
                input: PathBuf::from(&object.name),
                import: None,
                oriented: None,
                hollow: saved_hollow(manifest, object, meshes),
                drains: None,
                supports: saved_supports(object, model),
                standing: bounds(model).map(|bounds| FitCheck::of(&bounds, &built.printer)),
                meets: Vec::new(),
                prepared: None,
                recipe: None,
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
        saved: None,
    })
}

/// What the cavity `object` carries as saved says of it, as `--hollow` would have.
fn saved_hollow(
    manifest: &Manifest,
    object: &ObjectState,
    meshes: &ModelMeshes,
) -> Option<HollowReport> {
    let built = object.hollow.built.as_ref()?;
    let state = object.own_hollow.as_ref().unwrap_or(&manifest.hollow);
    let faces = (meshes.source.faces.len(), meshes.printed().faces.len());
    let lattice = (built.cavity_mm3, built.voxel_mm, built.coarsened);
    Some(HollowReport::of(
        (state, object.transform),
        faces,
        lattice,
        Duration::ZERO,
    ))
}

/// What the supports `object` stands on as saved say of them, as `--supports` would have.
fn saved_supports(object: &ObjectState, model: &core_engine::Model) -> Option<SupportReport> {
    let held = &object.supports;
    let trees: Vec<_> = held.frozen.iter().chain(&held.grown).collect();
    if trees.is_empty() {
        return None;
    }
    let lowest = placed_aabb(&model.mesh, model.transform).map_or(0.0, |bounds| bounds.mins.z);
    Some(SupportReport {
        lifted_mm: lowest.max(0.0),
        contacts: trees.iter().flat_map(|tree| tree.contacts()).collect(),
        standing: trees.len(),
        unheld: Vec::new(),
        trees: trees.len(),
        faces: model.supports.iter().map(|mesh| mesh.faces.len()).sum(),
    })
}

/// The printer and resin the flags name, or else the ones the project names by id, as
/// the user's catalogue has them; the engine tunes the resin to the machine as the window does.
pub(crate) fn machine(manifest: &Manifest, job: &JobArgs) -> Result<Machine> {
    let flags = &job.profile;
    let (printer, printer_id) = match (&flags.profile, flags.printer_id()) {
        (Some(path), _) => {
            let printer = PrinterProfile::load(path)
                .with_context(|| format!("cannot load {}", path.display()))?;
            (printer, None)
        }
        (None, Some(id)) => {
            let why = format!("--printer {id} is not among your printers");
            (printer_of(id, &why)?, Some(id.to_owned()))
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
    let mut resin = match (&flags.material, &flags.resin) {
        (Some(path), _) => MaterialProfile::load(path)
            .with_context(|| format!("cannot load {}", path.display()))?,
        (None, Some(id)) => resin_of(id, &format!("--resin {id} is not among your resins"))?,
        (None, None) if flags.standard_machine => MaterialProfile::default(),
        (None, None) => {
            let id = manifest
                .resin
                .as_deref()
                .context("the project names no resin: pick one with --resin")?;
            let why = format!("the project's resin {id} is not among yours: pick one with --resin");
            resin_of(id, &why)?
        }
    };
    flags.set.apply(&mut resin);
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

/// The flags a project cannot take: a recipe and Check's answers, which prepare models as they
/// come in, the islands it slices as saved, and models combined from files.
pub fn refuse_for_projects(job: &JobArgs) -> Result<()> {
    job.hollow.check_flags()?;
    let refused = [
        ("--prepare", job.prepare),
        ("--recipe", job.recipe.is_some()),
        (
            "--support-islands, --drill-cups and --hold-necks",
            job.fixes.step()?.is_some(),
        ),
        ("--remove-islands", job.raster.remove_islands),
        ("--hanging-passes", job.raster.hanging_passes != 0),
        ("--combine", job.model.combine.is_some()),
    ];
    if let Some((flag, _)) = refused.iter().find(|(_, given)| *given) {
        bail!("{flag} does not apply to a project; prepare the models it holds by their flags");
    }
    Ok(())
}

/// Whether a flag given shapes the project's models, so its plate is edited by them first.
pub fn shaped_by_flags(job: &JobArgs, arrange: bool) -> bool {
    let import = &job.import;
    let transform = &import.transform;
    arrange
        || !job.only.is_empty()
        || job.model.wanted()
        || import.repair.is_some()
        || !crate::edits::mend_asked(import).is_empty()
        || import.relief.is_some()
        || transform.places()
        || transform.placing.copies.is_some()
        || job.hollow.wanted()
        || job.hollow.cutting()
        || job.supports.supports.is_some()
        || job.supports.support_profile.is_some()
        || job.supports.support_cavity
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
