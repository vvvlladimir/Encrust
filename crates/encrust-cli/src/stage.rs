//! A plate assembled from what the command line was given — one model, several, a plate
//! file or a project — before it is written or measured.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use core_engine::{Cutting, Drained, Model, Plate, SupportPlan};
use core_format::ExposurePlan;
use core_geometry::{Aabb, BoundingVolume, Mesh, Scalar, Vec2, placed_aabb};
use core_pipeline::{PanelOverrides, SlicedFormat};
use printer_profiles::{MaterialProfile, PrinterProfile};

use crate::args::{ImportArgs, JobArgs, untyped};
use crate::hollowing::{HollowArgs, HollowReport};
use crate::modelling::ModelArgs;
use crate::pipeline::{
    OrientSummary, Watch, cutting_of, import, overrides_of, warn_if_grey_ignored,
};
use crate::profiles::{self, Chosen};
use crate::recipe::{self, Prepared, RecipeRun, Saved};
use crate::report::{FitCheck, ImportReport, describe_fit};
use crate::sliced_file::now_unix_s;
use crate::supports::SupportReport;
use crate::{meeting, plate_file, project};

mod document;

pub(crate) use document::{Staging, placed_by_edits, said};
pub(crate) use finished::{Finished, usable_area};
pub(crate) use placed::{shipped_printer, summary_of};
mod finished;
mod modelled;
mod placed;
mod repaired;
mod room;

pub(crate) use room::cleared;
use room::stood_at;

/// What is done to one model on its way onto the plate.
#[derive(Debug, Clone)]
pub struct Shaping {
    pub import: ImportArgs,
    pub hollow: HollowArgs,
    pub supports: Option<SupportPlan>,
    /// Where the middle of the model's footprint goes, plate millimetres, standing on the
    /// plate; `None` leaves it where the file and the flags put it.
    pub position: Option<Vec2>,
    /// Which of its hollow and supports a plate file's key gave rather than a flag.
    pub keyed: Keyed,
}

/// The plate file's keys that ask a step of a recipe to run, as their flags do.
#[derive(Debug, Clone, Copy, Default)]
pub struct Keyed {
    pub hollow: bool,
    pub supports: bool,
}

impl Shaping {
    /// What the flags ask of every model.
    pub fn of(job: &JobArgs) -> Result<Self> {
        Ok(Self {
            import: job.import.clone(),
            hollow: job.hollow.clone(),
            supports: job.supports.plan()?,
            position: None,
            keyed: Keyed::default(),
        })
    }

    /// What `import` asks of a model taken on its own, hollowing and supporting nothing.
    pub fn of_import(import: ImportArgs) -> Result<Self> {
        Ok(Self {
            import,
            hollow: untyped()?,
            supports: None,
            position: None,
            keyed: Keyed::default(),
        })
    }
}

/// One model's way onto the plate, and what each stage found on it.
pub struct Part {
    /// The file it came from, or its name in the project it was saved in.
    pub input: PathBuf,
    /// Absent for a model out of a project, which was repaired when it was imported.
    pub import: Option<ImportReport>,
    pub oriented: Option<OrientSummary>,
    pub hollow: Option<HollowReport>,
    /// What `--drain-at auto` found and drilled, where the model stood before its supports.
    pub drains: Option<Drained>,
    pub supports: Option<SupportReport>,
    /// How the model with its supports and raft sits on the plate where it was put down.
    pub standing: Option<FitCheck>,
    /// The inputs of the other parts it, or its supports, meet (ADR 0392).
    pub meets: Vec<PathBuf>,
    /// What `--prepare` decided for it.
    pub prepared: Option<Prepared>,
    /// What each step of the recipe it was prepared by decided.
    pub recipe: Option<RecipeRun>,
}

impl Part {
    /// Nothing found on this model would stop it printing correctly.
    pub fn is_clean(&self) -> bool {
        self.import.as_ref().is_none_or(ImportReport::is_clean)
            && self.hollow.as_ref().is_none_or(HollowReport::is_clean)
            && self.standing.as_ref().is_none_or(|check| check.fit.fits())
            && self.meets.is_empty()
    }

    /// How the model sits on the plate: where it was put down, or else as imported.
    pub fn fit(&self) -> Option<&FitCheck> {
        let imported = self.import.as_ref().and_then(|import| import.fit.as_ref());
        self.standing.as_ref().or(imported)
    }
}

/// Everything a run needs but the output: the models standing where they print, the
/// machine and resin, and how the stack is cut and drawn.
pub struct Staged {
    pub parts: Vec<Part>,
    pub models: Vec<Model>,
    pub printer: Option<PrinterProfile>,
    pub material: MaterialProfile,
    pub cutting: Cutting,
    pub panel: PanelOverrides,
    pub exposure: ExposurePlan,
    pub remove_islands: bool,
    /// Layers more taken of what is left hanging over a removed island.
    pub hanging_passes: u32,
    /// Whether the stack is watched for resin with no way out.
    pub drainage: bool,
    /// The plate as a recipe's last save step wrote it.
    pub saved: Option<Saved>,
}

impl Staged {
    /// The run's input for a file of `format` (default: the printer's), or `None` with no printer.
    /// The models move into the plate so the caller can free them before the write.
    pub fn take_plate(
        &mut self,
        format: Option<SlicedFormat>,
        raster_window: usize,
    ) -> Option<Plate> {
        let printer = self.printer.clone()?;
        let format = format
            .unwrap_or_else(|| printer.output.into())
            .at_revision_of(printer.output);
        Some(Plate {
            models: std::mem::take(&mut self.models),
            material: self.material.clone(),
            panel: self.panel,
            cutting: self.cutting,
            exposure: self.exposure.clone(),
            remove_islands: self.remove_islands,
            hanging_passes: self.hanging_passes,
            format,
            raster_window,
            created_unix_s: now_unix_s(),
            printer,
        })
    }
}

/// What the inputs on the command line are: models, or one file describing a plate.
enum Given<'a> {
    Models(&'a [PathBuf]),
    PlateFile(&'a Path),
    Project(&'a Path),
}

fn given(inputs: &[PathBuf]) -> Result<Given<'_>> {
    let plate = inputs.iter().find(|path| describes_a_plate(path));
    match (plate, inputs) {
        (None, []) => bail!("nothing to slice: name a model, a plate file or a project"),
        (None, _) => Ok(Given::Models(inputs)),
        (Some(_), [only]) if extension_of(only).as_deref() == Some("toml") => {
            Ok(Given::PlateFile(only))
        }
        (Some(_), [only]) => Ok(Given::Project(only)),
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

/// Stages whatever `inputs` name. `arrange` spreads several models over the plate; `plate`
/// picks one of a project's, counted from one.
pub fn stage(
    inputs: &[PathBuf],
    arrange: bool,
    plate: Option<u32>,
    job: &JobArgs,
    watch: &Watch,
) -> Result<Staged> {
    let given = given(inputs)?;
    if plate.is_some() && !matches!(given, Given::Project(_)) {
        bail!("--plate picks a plate of a project (.encrust)");
    }
    match given {
        Given::Models(paths) => {
            let chosen = profiles::resolve(&job.profile.selection())?;
            models(paths, arrange, job, &chosen, watch)
        }
        Given::PlateFile(path) => {
            job.edits.refuse_for_models()?;
            plate_file::stage(path, arrange, job, watch)
        }
        Given::Project(path) => {
            refuse_for_a_project(arrange, job)?;
            let project = job.edits.applied(project::open(path)?)?;
            project::stage_checked(path, &project, plate, job, watch)
        }
    }
}

/// Names each model of a saved plate that does not stand inside the build volume.
pub(crate) fn print_off_the_plate(parts: &[Part]) {
    let off = parts.iter().filter_map(|part| {
        let check = part.standing.as_ref().filter(|check| !check.fit.fits())?;
        Some((part, check))
    });
    for (part, check) in off {
        println!(
            "{}\n  fits           {}\n",
            part.input.display(),
            describe_fit(check)
        );
    }
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

/// Every model in `paths`, shaped as the flags ask.
pub fn models(
    paths: &[PathBuf],
    arrange: bool,
    job: &JobArgs,
    chosen: &Chosen,
    watch: &Watch,
) -> Result<Staged> {
    let shaping = Shaping::of(job)?;
    refuse_idle_cavity(job, &shaping)?;
    if paths.len() > 1 {
        refuse_one_model_flags(&shaping, job.model.combine.is_some())?;
        let one_spot = job.model.label.is_some() || job.model.part_off.is_some();
        if one_spot && job.model.combine.is_none() {
            bail!("--label-at and --part-off point at one model; --combine makes the models one");
        }
    }
    let entries: Vec<_> = paths
        .iter()
        .map(|path| (path.clone(), shaping.clone()))
        .collect();
    let layer_height = job
        .slicing
        .layer_height
        .unwrap_or(chosen.material.layer_height_mm);
    plate_of(&entries, arrange, layer_height, job, chosen, watch)
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
fn refuse_idle_cavity(job: &JobArgs, shaping: &Shaping) -> Result<()> {
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

/// Imports, places, hollows and supports every entry, or takes each through the recipe
/// asked for, and stages the plate they make.
pub fn plate_of(
    entries: &[(PathBuf, Shaping)],
    arrange: bool,
    layer_height: Scalar,
    job: &JobArgs,
    chosen: &Chosen,
    watch: &Watch,
) -> Result<Staged> {
    chosen.measured()?;
    let printer = chosen.printer.as_ref();
    check_adaptive(job, printer)?;
    let asked = recipe::Asked::of(job)?;
    if asked.is_some() && job.edits.wanted() {
        bail!("--edits and --project-out take the flags' plate, and a recipe prepares its own");
    }
    let (staging, found) = document::shaped(entries, (job, chosen), watch, asked.is_some())?;
    let pieces = staging.pieces(entries.len());
    check_pieces(&pieces, entries, (&job.model, arrange), asked.is_some())?;
    watch.stop.check()?;

    let drift_mm = chosen.material.compensation.fit_drift_mm();
    let held = (printer, drift_mm);
    let (laid, saved) = match &asked {
        Some(asked) => {
            let ctx = recipe::Ctx {
                job,
                chosen,
                layer_height_mm: layer_height,
                watch,
            };
            let left = recipe::lay(asked, (staging, found), entries, &ctx)?;
            let mut staging = left.staging;
            if arrange {
                document::arranged(&mut staging, entries, printer, watch)?;
            }
            let reported = (left.found, left.finished);
            let mut laid = staging.laid(reported, (entries, None), printer, watch.talk)?;
            for (part, said) in laid.parts.iter_mut().zip(left.said) {
                if let Some((prepared, run)) = said {
                    part.prepared = prepared;
                    part.recipe = Some(run);
                }
            }
            (laid, left.saved)
        }
        None => {
            let edits = (entries, &job.edits);
            (
                document::laid(staging, found, edits, (held, arrange), watch)?,
                None,
            )
        }
    };
    let Laid {
        mut parts,
        models,
        owners,
        ..
    } = laid;
    meeting::mark(&mut parts, &models, &owners, watch.talk);

    if watch.talk {
        warn_if_grey_ignored(&job.raster, chosen.printer.as_ref());
    }
    let drainage = job.slicing.check_drainage
        || parts.iter().any(|part| part.hollow.is_some())
        || entries
            .iter()
            .any(|(_, shaping)| shaping.hollow.wanted() || shaping.hollow.cutting());
    let machine = (job, chosen, layer_height);
    let mut staged = staged((parts, models), drainage, machine, watch.talk);
    staged.saved = saved;
    Ok(staged)
}

/// `models` staged to be cut for `parts` on the machine and at the layer height a run asks,
/// watched for trapped resin where `drainage` says.
pub(crate) fn staged(
    (parts, models): (Vec<Part>, Vec<Model>),
    drainage: bool,
    (job, chosen, layer_height): (&JobArgs, &Chosen, Scalar),
    talk: bool,
) -> Staged {
    Staged {
        parts,
        models,
        printer: chosen.printer.clone(),
        material: chosen.material.clone(),
        cutting: cutting_of(job, layer_height, &chosen.material.compensation, talk),
        panel: overrides_of(&job.raster, chosen.material.edges),
        exposure: ExposurePlan::new(job.slicing.exposure_at.clone()),
        remove_islands: job.raster.remove_islands,
        hanging_passes: job.raster.hanging_passes,
        drainage,
        saved: None,
    }
}

/// What importing one model reported, and how it was turned.
pub(crate) type Found = (ImportReport, Option<OrientSummary>);

/// Each entry's model imported and stood where it asks, with what its import reported, all
/// it asks after the import left to edits.
fn imported(
    entries: &[(PathBuf, Shaping)],
    chosen: &Chosen,
    watch: &Watch,
) -> Result<(Vec<Mesh>, Vec<Found>)> {
    let mut meshes = Vec::with_capacity(entries.len());
    let mut found = Vec::with_capacity(entries.len());
    for (path, shaping) in entries {
        let mut asked = shaping.import.clone();
        repaired::left_to_edits(&mut asked);
        placed::left_to_edits(&mut asked.transform);
        let (mut mesh, import) = import(path, &asked, chosen, watch)?;
        let oriented = None;
        if let Some(position) = shaping.position {
            mesh = stood_at(&mesh, position)?;
        }
        meshes.push(mesh);
        found.push((import, oriented));
        watch.stop.check()?;
    }
    Ok((meshes, found))
}

/// The box `model` and its supports stand in, plate millimetres.
pub(crate) fn bounds(model: &Model) -> Option<Aabb> {
    let supports = model.support_meshes().filter_map(|mesh| mesh.aabb());
    placed_aabb(&model.mesh, model.transform)
        .into_iter()
        .chain(supports)
        .reduce(|one, other| one.merged(&other))
}

/// Every model of a plate put down, and which part each model, plug or piece is of.
pub(crate) struct Laid {
    parts: Vec<Part>,
    models: Vec<Model>,
    /// The part each of `models` belongs to, by its index in `parts`.
    owners: Vec<usize>,
}

/// How a model put down in `stands_in` sits on `printer`'s plate, said when supports changed
/// what stands there or it does not fit.
pub(crate) fn standing_of(
    stands_in: Option<Aabb>,
    printer: Option<&PrinterProfile>,
    supported: bool,
    talk: bool,
) -> Option<FitCheck> {
    let check = FitCheck::of(&stands_in?, printer?);
    if talk && (supported || !check.fit.fits()) {
        println!("On the plate\n  fits           {}\n", describe_fit(&check));
    }
    Some(check)
}

/// Refuses what the rest of the run cannot take: several pieces of one model to hollow, support
/// or prepare by a recipe, or halves of a cut standing on one another's spot.
fn check_pieces(
    pieces: &[Vec<Mesh>],
    entries: &[(PathBuf, Shaping)],
    (model, arrange): (&ModelArgs, bool),
    prepared: bool,
) -> Result<()> {
    if model.cut.is_some() && !arrange {
        bail!("--cut lays the halves down where they would stand on each other; add --arrange");
    }
    for (meshes, (path, shaping)) in pieces.iter().zip(entries) {
        let shaped = prepared
            || shaping.hollow.wanted()
            || shaping.hollow.cutting()
            || shaping.supports.is_some();
        if meshes.len() > 1 && shaped {
            bail!(
                "{} comes out of --cut or --split in {} pieces; hollow or support them from a plate file of the pieces",
                path.display(),
                meshes.len()
            );
        }
    }
    Ok(())
}

/// Refuses an adaptive run on a machine that steps the plate by the header's layer
/// height, before the stack is cut rather than when the file comes to be written.
fn check_adaptive(job: &JobArgs, printer: Option<&PrinterProfile>) -> Result<()> {
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

/// Whether anything done to this model past import has a report of its own to print.
fn shapes_anything(shaping: &Shaping) -> bool {
    shaping.hollow.wanted() || shaping.hollow.cutting() || shaping.supports.is_some()
}
