//! A plate assembled from what the command line was given — one model, several, a plate
//! file or a project — before it is written or measured.

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use core_engine::{Cutting, Model, Plate, SupportPlan};
use core_format::ExposurePlan;
use core_geometry::{Mesh, Scalar, Transform, Vec2, Vec3, transform_mesh};
use core_pipeline::{PanelOverrides, SlicedFormat};
use core_plate::{ArrangeSettings, Footprint, arrange};
use printer_profiles::{MaterialProfile, PrinterProfile};

use crate::args::{ImportArgs, JobArgs};
use crate::hollowing::{HollowArgs, HollowReport, support_holes};
use crate::modelling::{self, ModelArgs};
use crate::pipeline::{OrientSummary, Watch, cutting_of, hollow_and_cut, import, overrides_of};
use crate::profiles::{self, Chosen};
use crate::report::ImportReport;
use crate::sliced_file::now_unix_s;
use crate::supports::{SupportReport, stand_under};
use crate::{plate_file, project};

/// What is done to one model on its way onto the plate.
#[derive(Debug, Clone)]
pub struct Shaping {
    pub import: ImportArgs,
    pub hollow: HollowArgs,
    pub supports: Option<SupportPlan>,
    /// Where the middle of the model's footprint goes, plate millimetres, standing on the
    /// plate; `None` leaves it where the file and the flags put it.
    pub position: Option<Vec2>,
}

impl Shaping {
    /// What the flags ask of every model.
    pub fn of(job: &JobArgs) -> Result<Self> {
        Ok(Self {
            import: job.import.clone(),
            hollow: job.hollow.clone(),
            supports: job.supports.plan()?,
            position: None,
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
    pub supports: Option<SupportReport>,
}

impl Part {
    /// Nothing found on this model would stop it printing correctly.
    pub fn is_clean(&self) -> bool {
        self.import.as_ref().is_none_or(ImportReport::is_clean)
            && self.hollow.as_ref().is_none_or(HollowReport::is_clean)
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

/// Whether `path` names a whole plate, a plate file or a project, rather than a model.
pub fn describes_a_plate(path: &Path) -> bool {
    matches!(extension_of(path).as_deref(), Some("toml" | "encrust"))
}

fn extension_of(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
}

/// Stages whatever `inputs` name. `arrange` spreads several models over the plate.
pub fn stage(inputs: &[PathBuf], arrange: bool, job: &JobArgs, watch: &Watch) -> Result<Staged> {
    match given(inputs)? {
        Given::Models(paths) => {
            let chosen = profiles::resolve(&job.profile.selection())?;
            models(paths, arrange, job, &chosen, watch)
        }
        Given::PlateFile(path) => plate_file::stage(path, arrange, job, watch),
        Given::Project(path) => {
            if job.model.wanted() {
                bail!("a project's models are combined, cut and split in the window");
            }
            if arrange {
                bail!("a project stands where it was saved; arrange it in the window");
            }
            project::stage(path, job)
        }
    }
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
        refuse_one_model_flags(&shaping)?;
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

/// Flags that point at one spot on one model mean nothing across several.
pub fn refuse_one_model_flags(shaping: &Shaping) -> Result<()> {
    if shaping.import.transform.center {
        bail!("--center stands every model in the middle of the plate; --arrange spreads them");
    }
    if shaping.hollow.cutting() {
        bail!("--drain-at and --channel point at one model; a plate file places them per model");
    }
    Ok(())
}

/// Refuses `--support-cavity` where it would hold nothing up: no supports, or no cavity.
fn refuse_idle_cavity(job: &JobArgs, shaping: &Shaping) -> Result<()> {
    if !job.supports.support_cavity {
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

/// Imports, places, hollows and supports every entry, and stages the plate they make.
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
    let mut meshes = Vec::with_capacity(entries.len());
    let mut found = Vec::with_capacity(entries.len());
    for (path, shaping) in entries {
        let (mut mesh, import, oriented) = import(
            path,
            &shaping.import,
            shaping.supports.as_ref(),
            chosen,
            watch.talk,
        )?;
        if let Some(position) = shaping.position {
            mesh = stood_at(&mesh, position)?;
        }
        meshes.push(mesh);
        found.push((import, oriented));
        watch.stop.check()?;
    }
    let mut pieces = modelling::shaped(meshes, &job.model, watch.talk)?;
    check_pieces(&pieces, entries, &job.model, arrange)?;
    watch.stop.check()?;
    if arrange {
        let mut spread_out: Vec<(&mut Mesh, &Path)> = pieces
            .iter_mut()
            .zip(entries)
            .flat_map(|(meshes, (path, _))| {
                meshes.iter_mut().map(move |mesh| (mesh, path.as_path()))
            })
            .collect();
        spread(&mut spread_out, printer)?;
    }

    let mut parts = Vec::with_capacity(entries.len());
    let mut models = Vec::with_capacity(entries.len());
    let mut plugged = Vec::new();
    let drift_mm = chosen.material.compensation.fit_drift_mm();
    for ((meshes, (import, oriented)), (path, shaping)) in
        pieces.into_iter().zip(found).zip(entries)
    {
        // The imports are all reported before the first model is shaped, so on a plate
        // these lines would otherwise read as the last model's.
        if watch.talk && entries.len() > 1 && shapes_anything(shaping) {
            println!("{}", path.display());
        }
        let seated = oriented.as_ref().is_some_and(|turn| turn.on_plate);
        let (mut hollow, mut supports) = (None, None);
        for mesh in meshes {
            let down = put_down(mesh, shaping, seated, layer_height, drift_mm, watch)?;
            models.push(down.model);
            let first = models.len();
            models.extend(down.plugs);
            plugged.push((first..models.len(), path.as_path()));
            hollow = hollow.or(down.hollow);
            supports = supports.or(down.supports);
        }
        parts.push(Part {
            input: path.clone(),
            import: Some(import),
            oriented,
            hollow,
            supports,
        });
    }

    check_plugs_clear(&models, &plugged, printer)?;

    let drainage = job.slicing.check_drainage
        || entries
            .iter()
            .any(|(_, shaping)| shaping.hollow.wanted() || shaping.hollow.cutting());
    Ok(Staged {
        parts,
        models,
        printer: chosen.printer.clone(),
        material: chosen.material.clone(),
        cutting: cutting_of(job, layer_height, &chosen.material.compensation, watch.talk),
        panel: overrides_of(&job.raster),
        exposure: ExposurePlan::new(job.slicing.exposure_at.clone()),
        remove_islands: job.raster.remove_islands,
        hanging_passes: job.raster.hanging_passes,
        drainage,
    })
}

/// Hollows and supports one piece as `shaping` asks, ready to slice, with the plugs for
/// its holes standing beside it on no supports of their own.
fn put_down(
    mut mesh: Mesh,
    shaping: &Shaping,
    seated: bool,
    layer_height: Scalar,
    drift_mm: Scalar,
    watch: &Watch,
) -> Result<PutDown> {
    let shelled = hollow_and_cut(
        &mut mesh,
        &shaping.hollow,
        shaping.import.precision,
        drift_mm,
        watch,
    )?;
    let (hollow, cavity, mut cuts) = (shelled.report, shelled.cavity, shelled.cuts);
    let plugs = shelled
        .plugs
        .into_iter()
        .map(|plug| Model {
            mesh: Arc::new(plug),
            transform: Transform::default(),
            cuts: None,
            drilled: None,
            supports: Vec::new(),
        })
        .collect();
    watch.stop.check()?;
    let stood = shaping
        .supports
        .as_ref()
        .map(|plan| {
            stand_under(
                &mut mesh,
                cavity.clone(),
                &mut cuts,
                plan,
                layer_height,
                seated,
            )
        })
        .transpose()?
        .filter(|(report, _)| !seated || !report.contacts.is_empty());
    if !shaping.hollow.drain_support_at.is_empty() {
        let Some((_, columns)) = &stood else {
            bail!("--drain-support-at drills into the supports, and none stand under the model");
        };
        let drilled = support_holes(columns, &shaping.hollow)?;
        cuts.get_or_insert_default().append(&drilled);
    }
    let (supports, columns) = match stood {
        Some((report, mesh)) => (Some(report), vec![Arc::new(mesh)]),
        None => (None, Vec::new()),
    };
    if watch.talk
        && let Some(report) = &supports
    {
        print!("{report}");
    }
    watch.stop.check()?;
    let model = Model {
        mesh: Arc::new(mesh),
        transform: Transform::default(),
        cuts: cuts.map(Arc::new),
        drilled: None,
        supports: columns,
    };
    Ok(PutDown {
        model,
        plugs,
        hollow,
        supports,
    })
}

/// Refuses plugs that land off the plate or on a model: laid in a row past their own
/// model, they know nothing of the plate's edge or of its neighbours.
fn check_plugs_clear(
    models: &[Model],
    plugged: &[(Range<usize>, &Path)],
    printer: Option<&PrinterProfile>,
) -> Result<()> {
    let footprints: Vec<Option<(Vec2, Vec2)>> = models.iter().map(footprint).collect();
    for (plugs, path) in plugged {
        for (mins, maxs) in footprints[plugs.clone()].iter().flatten() {
            let off = printer.is_some_and(|printer| {
                let plate = Vec2::new(printer.build_volume.x, printer.build_volume.y);
                mins.cmplt(Vec2::ZERO).any() || maxs.cmpgt(plate).any()
            });
            if off {
                bail!(
                    "the plugs for {} run off the plate; move the model left or leave out --drain-plugs",
                    path.display()
                );
            }
            let crowded = footprints.iter().enumerate().any(|(index, other)| {
                !plugs.contains(&index)
                    && other
                        .is_some_and(|(low, high)| mins.cmplt(high).all() && low.cmplt(*maxs).all())
            });
            if crowded {
                bail!(
                    "the plugs for {} land on another model; give it room on its right or leave out --drain-plugs",
                    path.display()
                );
            }
        }
    }
    Ok(())
}

/// What `model` and its supports cover of the plate, its least and greatest corners.
fn footprint(model: &Model) -> Option<(Vec2, Vec2)> {
    std::iter::once(&model.mesh)
        .chain(&model.supports)
        .filter_map(|mesh| mesh.aabb())
        .map(|bounds| (bounds.mins.truncate(), bounds.maxs.truncate()))
        .reduce(|(low, high), (mins, maxs)| (low.min(mins), high.max(maxs)))
}

/// One piece ready to slice, the plugs for its holes, and what hollowing and supporting
/// it found.
struct PutDown {
    model: Model,
    plugs: Vec<Model>,
    hollow: Option<HollowReport>,
    supports: Option<SupportReport>,
}

/// Refuses what the rest of the run cannot take: several pieces of one model to hollow or support,
/// or halves of a cut standing on one another's spot.
fn check_pieces(
    pieces: &[Vec<Mesh>],
    entries: &[(PathBuf, Shaping)],
    model: &ModelArgs,
    arrange: bool,
) -> Result<()> {
    if model.cut.is_some() && !arrange {
        bail!("--cut lays the halves down where they would stand on each other; add --arrange");
    }
    for (meshes, (path, shaping)) in pieces.iter().zip(entries) {
        let shaped =
            shaping.hollow.wanted() || shaping.hollow.cutting() || shaping.supports.is_some();
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

/// The model moved so the middle of its footprint is over `position` and its lowest
/// point is on the plate.
fn stood_at(mesh: &Mesh, position: Vec2) -> Result<Mesh> {
    let bounds = mesh.aabb().context("mesh has no vertices")?;
    let middle = (bounds.mins.truncate() + bounds.maxs.truncate()) / 2.0;
    let offset = (position - middle).extend(-bounds.mins.z);
    Ok(transform_mesh(mesh, Transform::from_translation(offset)))
}

/// Packs the models over the plate, biggest first, and refuses a plate they do not fit.
fn spread(placed: &mut [(&mut Mesh, &Path)], printer: Option<&PrinterProfile>) -> Result<()> {
    let printer = printer.context("--arrange needs a printer to know the plate")?;
    let settings = ArrangeSettings::default();
    let footprints = placed
        .iter()
        .map(|(mesh, path)| {
            Footprint::of(mesh, Transform::default(), settings.cell_mm)
                .with_context(|| format!("{} has no footprint", path.display()))
        })
        .collect::<Result<Vec<_>>>()?;
    let plate = Vec2::new(printer.build_volume.x, printer.build_volume.y);
    let arranged = arrange(&footprints, plate, &settings);
    if let Some(&first) = arranged.left_out.first() {
        bail!(
            "{} of {} models found no room on the plate, {} among them",
            arranged.left_out.len(),
            placed.len(),
            placed[first].1.display()
        );
    }
    for spot in &arranged.placed {
        let mesh = &mut *placed[spot.index].0;
        let offset = Vec3::new(spot.offset_mm.x, spot.offset_mm.y, 0.0);
        *mesh = transform_mesh(mesh, Transform::from_translation(offset));
    }
    Ok(())
}
