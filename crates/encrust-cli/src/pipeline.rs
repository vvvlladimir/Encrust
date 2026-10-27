//! The stages one model goes through onto the plate, and the cut of a staged plate into a file.
//! The single run, the batch and a multi-model plate share it; `stage` assembles a plate.

use std::ops::Range;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use core_analysis::Measured;
use core_engine::{Cutting, SupportPlan, bake, cut};
use core_geometry::{Bvh, Mesh, Scalar, self_intersections, weld};
use core_mesh_io::{Loaded, loader_for_extension};
use core_pipeline::{PanelOverrides, Tolerance, raster_settings};
use core_raster::Shading;
use core_slicer::{AdaptiveSettings, Solid, Windows};
use printer_profiles::{Compensation, MaterialProfile};

use crate::args::{ImportArgs, JobArgs, RasterArgs};
use crate::exit::Stop;
use crate::hollowing::{self, HollowArgs, HollowReport};
use crate::png_stack::write_stack;
use crate::profiles::Chosen;
use crate::raster_report::RasterReport;
use crate::report::ImportReport;
use crate::slice_report::SliceReport;
use crate::sliced_file::{asked_revision, format_of, write_plate};
use crate::stage::{self, Part, Staged};

mod checked;
mod place;
mod relief;
use checked::checked;
pub use place::{OrientSummary, orient_settings};
use place::{centred, place};
use relief::press_relief;

/// How a run talks while it works, and what can stop it.
pub struct Watch<'a> {
    /// Print each stage as it finishes, which a single run wants and a batch does not: a
    /// folder of two hundred models would bury its own summary.
    pub talk: bool,
    /// Draw a bar over the layers as they are written.
    pub progress: bool,
    pub stop: &'a Stop,
}

/// What one run found and wrote: a part per model on the plate, and the stack they made.
pub struct Outcome {
    pub parts: Vec<Part>,
    pub output: PathBuf,
    pub slice: SliceReport,
    pub raster: Option<RasterReport>,
}

impl Outcome {
    /// Nothing found would stop this plate printing correctly.
    pub fn is_clean(&self) -> bool {
        self.parts.iter().all(Part::is_clean)
            && self.slice.is_clean()
            && self.raster.as_ref().is_none_or(RasterReport::is_clean)
    }
}

/// Loads, repairs, places, hollows, supports and slices one model into `output`, asking
/// `watch.stop` between stages and between windows of layers.
pub fn slice_one(
    input: &Path,
    output: &Path,
    job: &JobArgs,
    chosen: &Chosen,
    watch: &Watch,
) -> Result<Outcome> {
    let mut staged = stage::models(&[input.to_path_buf()], false, job, chosen, watch)?;
    let (slice, raster) = slice_staged(&mut staged, output, job, watch)?;
    Ok(Outcome {
        parts: staged.parts,
        output: output.to_path_buf(),
        slice,
        raster,
    })
}

/// Loads, welds, places and repairs the model, and says what import found. `supports` is
/// what it will stand on, if anything, which is what an orientation is judged on.
pub fn import(
    input: &Path,
    args: &ImportArgs,
    supports: Option<&SupportPlan>,
    chosen: &Chosen,
    talk: bool,
) -> Result<(Mesh, ImportReport, Option<OrientSummary>)> {
    let profile = chosen.printer.as_ref();
    let loaded = load(input)?;
    if talk {
        talk_mapping(&loaded);
    }
    let welded = weld(&loaded.mesh, args.weld_tolerance);
    let (mesh, oriented) = place(welded.mesh.clone(), args, supports, chosen)?;
    if talk && let Some(summary) = &oriented {
        println!(
            "Oriented: {}, {:.1} mm2 largest section, {:.1} mm2 overhang{}\n",
            summary.describe(),
            summary.peak_mm2,
            summary.overhang_mm2,
            if summary.on_plate {
                ", set on the plate"
            } else {
                ""
            }
        );
    }

    // After placement, so depth in mm is the plate's; before the orientation fix, which rewinds
    // the faces the map is indexed by.
    let mut mesh = press_relief(mesh, &welded, &loaded, args, talk)?;
    if args.transform.center {
        mesh = centred(mesh, profile)?;
    }
    let import = checked(&mut mesh, input, welded, args, profile, false)?;
    if talk {
        print!("{import}");
    }
    Ok((mesh, import, oriented))
}

/// What hollowing and drilling made of one model besides the model itself.
pub struct Shelled {
    pub report: Option<HollowReport>,
    /// Which faces of the shell bound its cavity, when the model was hollowed.
    pub cavity: Option<Range<usize>>,
    /// The bodies the holes and channels cut, which only subtract, so they are no part of
    /// what the stack has to be tall enough for (ADR 0199).
    pub cuts: Option<Mesh>,
    /// A plug for each hole, laid on the plate beside the model (ADR 0313).
    pub plugs: Vec<Mesh>,
}

/// Shells the model when asked, then meshes the bodies its holes and channels cut, and the plugs.
/// `drift_mm` is what the resin leaves a fit wider by; see `Compensation::fit_drift_mm`.
pub fn hollow_and_cut(
    mesh: &mut Mesh,
    args: &HollowArgs,
    precision: Scalar,
    drift_mm: Scalar,
    watch: &Watch,
) -> Result<Shelled> {
    let talk = watch.talk;
    args.check_flags()?;
    // Where the cuts land is read off the model as it was imported; what they are cut
    // into may be the shell. See ADR 0075.
    let placed = args
        .cutting()
        .then(|| hollowing::placed(mesh, args))
        .transpose()?;

    let mut hollow = None;
    let mut shell = None;
    let mut cavity = None;
    if args.wanted() {
        let holes = placed.as_ref().map_or(&[][..], |(holes, _)| holes);
        let (shelled, faces, report) = hollowing::run(mesh, holes, args, precision, watch.stop)?;
        if talk {
            print!("{report}");
        }
        shell = Some(shelled);
        cavity = Some(faces);
        hollow = Some(report);
    }
    let wall = hollow.as_ref().map(HollowReport::wall);
    // A plug is the solid a hole takes out as it is cut, so it is read off the model before
    // the model is the shell.
    let plugs = match &placed {
        Some((holes, _)) if args.plugs => {
            let cut = hollowing::pierced(holes, wall);
            hollowing::plugs_beside(mesh, &cut, args, drift_mm)?
        }
        _ => Vec::new(),
    };
    if talk && !plugs.is_empty() {
        println!(
            "  plugs         {} beside the model, {:.2} mm narrower a side\n",
            plugs.len(),
            args.plug_gap_mm - drift_mm
        );
    }
    if let Some(shell) = shell {
        *mesh = shell;
    }
    let mut bodies = None;
    if let Some(placed) = placed {
        let (cuts, meshed) = hollowing::cut(placed, wall)?;
        if talk {
            print!("{cuts}");
        }
        bodies = Some(meshed);
    }
    Ok(Shelled {
        report: hollow,
        cavity,
        cuts: bodies,
        plugs,
    })
}

/// How the stack is cut, and what the resin's shrinkage does to it on the way in.
pub fn cutting_of(
    job: &JobArgs,
    layer_height: Scalar,
    compensation: &Compensation,
    talk: bool,
) -> Cutting {
    if talk && !compensation.scales_nothing() {
        println!(
            "Shrinkage: sliced at {:.3} % of X, {:.3} % of Y, {:.3} % of Z\n",
            compensation.shrink_x_pct, compensation.shrink_y_pct, compensation.shrink_z_pct
        );
    }
    Cutting {
        layer_height_mm: layer_height,
        adaptive: job
            .slicing
            .adaptive
            .then(|| adaptive_settings(job, layer_height, talk)),
        samples: job.slicing.samples_per_layer,
        compensation: *compensation,
        slice_window: job.slicing.slice_window,
    }
}

/// Cuts the staged plate and writes it, or only measures it when there is no panel to draw on.
/// A printable file is the engine's run; a PNG stack or no printer is cut here, no container.
pub fn slice_staged(
    staged: &mut Staged,
    output: &Path,
    job: &JobArgs,
    watch: &Watch,
) -> Result<(SliceReport, Option<RasterReport>)> {
    let format = format_of(output);
    let window = raster_window(&job.raster);
    if format.is_some() && staged.printer.is_none() {
        anyhow::bail!(
            "writing {} needs the machine it is for: name it with --printer or --profile",
            output.display()
        );
    }
    let plate = format
        .and_then(|format| staged.take_plate(Some(format), window))
        .map(|mut plate| {
            plate.format = asked_revision(plate.format, job.raster.ctb_version);
            plate
        });
    let (mut slice, raster) = match plate {
        Some(plate) => write_plate(plate, output, staged.drainage, watch)
            .map(|(slice, raster)| (slice, Some(raster)))?,
        None => cut_without_a_container(staged, output, window, watch.stop)?,
    };

    slice.finish_drainage();
    if watch.talk {
        print!("{slice}");
        if let Some(raster) = &raster {
            print!("{raster}");
        }
    }
    if job.strict {
        let clean = staged.parts.iter().all(Part::is_clean)
            && slice.is_clean()
            && raster.as_ref().is_none_or(RasterReport::is_clean);
        if !clean {
            discard(output, watch.talk);
        }
    }
    Ok((slice, raster))
}

/// Takes the output of a `--strict` run that found something off the disk, so the exit code and
/// what is left behind agree.
fn discard(output: &Path, talk: bool) {
    let taken = match output.is_dir() {
        true => std::fs::remove_dir_all(output),
        false => std::fs::remove_file(output),
    };
    match taken {
        Ok(()) => {
            if talk {
                println!("\n--strict: {} was not kept", output.display());
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => tracing::warn!("cannot remove {}: {error}", output.display()),
    }
}

/// The stack cut for a PNG directory, or only counted when no profile leaves nothing to
/// draw on.
fn cut_without_a_container(
    staged: &Staged,
    output: &Path,
    raster_window: usize,
    stop: &Stop,
) -> Result<(SliceReport, Option<RasterReport>)> {
    let material = &staged.material;
    let cutting = &staged.cutting;
    let baked = bake(&staged.models, &cutting.compensation).context("nothing to slice")?;
    let windows = cut(&baked, cutting)?;
    let solid = &baked.solid;
    let mut slice = report_of(solid, &windows, staged.drainage);

    let Some(profile) = staged.printer.as_ref() else {
        tracing::warn!("rasterisation needs --profile to know the panel; nothing was written");
        measure(solid, &windows, &mut slice, stop)?;
        return Ok((slice, None));
    };

    let settings = raster_settings(profile, staged.panel);
    settings
        .validate()
        .context("the panel cannot produce a usable mask")?;
    let report = write_stack(
        solid,
        &windows,
        &mut slice,
        &settings,
        output,
        &Tolerance::of(material),
        raster_window,
        fold_of(staged.remove_islands, staged.hanging_passes, material),
        stop,
    )
    .with_context(|| format!("cannot write the mask stack to {}", output.display()))?;
    Ok((slice, Some(report)))
}

/// An empty report over `windows`, watching the stack for trapped resin when `drainage`
/// asks for it.
pub fn report_of(solid: &Solid, windows: &Windows, drainage: bool) -> SliceReport {
    let mut slice = SliceReport::new(windows.settings(), windows.plan().clone());
    if drainage && let Some(bounds) = solid.aabb() {
        slice.watch_drainage(bounds.mins.truncate(), bounds.maxs.truncate());
    }
    slice
}

/// How the stack folds as it is written: how many bottom layers the plate holds, whether
/// islands come out, and how many layers more of what hung on them.
pub fn fold_of(remove_islands: bool, hanging_passes: u32, material: &MaterialProfile) -> Measured {
    let fold = Measured::new(material.bottom_layers as usize);
    if remove_islands {
        fold.removing_hanging(hanging_passes)
    } else {
        fold
    }
}

/// Layers rasterised at once, which defaults to one per thread.
pub fn raster_window(args: &RasterArgs) -> usize {
    args.raster_window
        .unwrap_or_else(rayon::current_num_threads)
}

/// Says what a file carried beyond its triangles, when it carried anything.
fn talk_mapping(loaded: &Loaded) {
    let Some(uvs) = &loaded.uvs else { return };
    let over = match uvs.mapped() {
        all if all == uvs.len() => format!("{all} faces"),
        some => format!("{some} of {} faces", uvs.len()),
    };
    let images: Vec<String> = loaded
        .textures
        .iter()
        .map(|texture| format!("{} ({} bytes)", texture.name, texture.bytes.len()))
        .collect();
    println!(
        "Textured: {} coordinates over {over}, from {}\n",
        uvs.mapped() * 3,
        images.join(", ")
    );
}

/// What `inspect` counts beyond what import always reports.
#[derive(Debug, Clone, Copy, Default)]
pub struct Inspect {
    /// The pairs of faces that cross.
    pub intersections: bool,
    /// The shells, as import leaves the model, which is how `--drop-shells` numbers them.
    pub shells: bool,
}

/// Loads and repairs a model without slicing it, for `inspect`, counting what `asked` names.
pub fn inspect(
    input: &Path,
    args: &ImportArgs,
    chosen: &Chosen,
    asked: Inspect,
) -> Result<ImportReport> {
    let profile = chosen.printer.as_ref();
    let loaded = load(input)?;
    let welded = weld(&loaded.mesh, args.weld_tolerance);
    let (mesh, _) = place(welded.mesh.clone(), args, None, chosen)?;
    // Pressed here too, so that what `inspect` measures is what would be printed.
    let mut mesh = press_relief(mesh, &welded, &loaded, args, false)?;
    if args.transform.center {
        mesh = centred(mesh, profile)?;
    }
    let mut report = checked(&mut mesh, input, welded, args, profile, asked.shells)?;
    if asked.intersections {
        report.crossing_pairs = Some(self_intersections(&mesh, &Bvh::build(&mesh)).len());
    }
    Ok(report)
}

/// What an adaptive run is allowed to do: the layer height becomes its ceiling.
fn adaptive_settings(job: &JobArgs, layer_height: Scalar, talk: bool) -> AdaptiveSettings {
    let settings = AdaptiveSettings {
        cusp_mm: job.slicing.cusp,
        // Half the layer height is a whole divisor of it whatever it is, so the thickest
        // layer the run can reach is the one that was asked for.
        min_height_mm: job.slicing.min_layer_height.unwrap_or(layer_height / 2.0),
        max_height_mm: layer_height,
    };
    let reachable = settings.reachable_max_mm();
    if talk && (reachable - layer_height).abs() > 1e-5 {
        tracing::warn!(
            "{layer_height} mm is not a whole number of {} mm layers, so the thickest \
             layer will be {reachable} mm and the exposure is scaled against that",
            settings.min_height_mm
        );
    }
    settings
}

/// What the command line sets over the panel the printer profile describes.
pub fn overrides_of(args: &RasterArgs) -> PanelOverrides {
    PanelOverrides {
        shading: if args.no_anti_alias {
            Shading::Binary
        } else {
            Shading::Coverage
        },
        grey_levels: args.grey_levels,
        grey_floor: args.grey_floor,
        blur_px: args.blur,
    }
}

/// Cuts the stack to count it, for a run that writes nothing.
fn measure(solid: &Solid, windows: &Windows, report: &mut SliceReport, stop: &Stop) -> Result<()> {
    windows
        .stream(solid, |sliced| {
            stop.check()?;
            report.absorb(sliced);
            Ok::<(), anyhow::Error>(())
        })
        .context("cannot slice the model")
}

pub fn load(path: &Path) -> Result<Loaded> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .with_context(|| format!("{} has no file extension", path.display()))?;

    let loader = loader_for_extension(extension)
        .with_context(|| format!("cannot load {}", path.display()))?;
    loader
        .load(path)
        .with_context(|| format!("cannot load {}", path.display()))
}

#[cfg(test)]
mod tests;
