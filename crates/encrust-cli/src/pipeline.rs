//! The stages one model goes through on its way onto the plate, and the cut of a staged
//! plate into a file.
//!
//! The single run, the batch and a plate of several models all go through here; what
//! assembles a plate is `stage`.

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result};
use core_analysis::Measured;
use core_engine::{Cutting, bake, cut};
use core_geometry::{
    Bvh, Mesh, Quat, Scalar, Transform, Vec3, Welded, center_over_plate, diagnose, drop_to_plate,
    orient_outward, transform_mesh, weld,
};
use core_mesh_io::{Loaded, loader_for_extension};
use core_pipeline::{PanelOverrides, Tolerance, raster_settings};
use core_plate::{OrientSettings, orient};
use core_raster::Shading;
use core_slicer::{AdaptiveSettings, Windows};
use core_volume::{ReliefSettings, press};
use printer_profiles::{Compensation, MaterialProfile, PrinterProfile};

use crate::args::{ImportArgs, JobArgs, RasterArgs};
use crate::exit::Stop;
use crate::hollowing::{self, HollowArgs, HollowReport};
use crate::png_stack::write_stack;
use crate::profiles::Chosen;
use crate::raster_report::RasterReport;
use crate::report::{FitCheck, ImportReport};
use crate::slice_report::SliceReport;
use crate::sliced_file::{format_of, write_plate};
use crate::stage::{self, Part, Staged};
use crate::stats::MeshStats;

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

/// The turn auto-orientation applied, and why it chose it.
pub struct OrientSummary {
    pub degrees: Scalar,
    pub peak_mm2: Scalar,
    pub overhang_mm2: Scalar,
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

/// Loads, welds, places and repairs the model, and says what import found.
pub fn import(
    input: &Path,
    args: &ImportArgs,
    profile: Option<&PrinterProfile>,
    talk: bool,
) -> Result<(Mesh, ImportReport, Option<OrientSummary>)> {
    let loaded = load(input)?;
    if talk {
        talk_mapping(&loaded);
    }
    let welded = weld(&loaded.mesh, args.weld_tolerance);
    let (mesh, oriented) = place(welded.mesh.clone(), args, profile)?;
    if talk && let Some(summary) = &oriented {
        println!(
            "Oriented: turned {:.0} degrees, {:.1} mm2 largest section, {:.1} mm2 overhang\n",
            summary.degrees, summary.peak_mm2, summary.overhang_mm2
        );
    }

    // After placement, so that a depth in millimetres is the plate's millimetre and not
    // whatever unit the file was drawn in; before the orientation fix, which renumbers
    // nothing but rewinds the faces the map is indexed by.
    let mut mesh = press_relief(mesh, &welded, &loaded, args, talk)?;
    let orientation = (!args.no_validate).then(|| orient_outward(&mut mesh));
    let diagnostics = (!args.no_validate).then(|| diagnose(&mesh));
    let stats = MeshStats::of(&mesh).context("mesh has no vertices")?;
    let fit = profile.map(|p| FitCheck::of(&stats, p));

    let import = ImportReport {
        path: input.to_path_buf(),
        stats,
        welded,
        diagnostics,
        orientation,
        fit,
    };
    if talk {
        print!("{import}");
    }
    Ok((mesh, import, oriented))
}

/// Shells the model when asked to, then meshes the bodies its drain holes and channels
/// cut out of it. Those stay apart from the model: they only subtract, so they are no
/// part of what the stack has to be tall enough for (ADR 0199).
pub fn hollow_and_cut(
    mesh: &mut Mesh,
    args: &HollowArgs,
    precision: Scalar,
    talk: bool,
) -> Result<(Option<HollowReport>, Option<Mesh>)> {
    // Where the cuts land is read off the model as it was imported; what they are cut
    // into may be the shell. See ADR 0075.
    let placed = args
        .cutting()
        .then(|| hollowing::placed(mesh, args))
        .transpose()?;

    let mut hollow = None;
    let mut wall = None;
    if args.wanted() {
        let (shelled, report) = hollowing::run(mesh, args, precision)?;
        if talk {
            print!("{report}");
        }
        wall = Some(report.wall());
        *mesh = shelled;
        hollow = Some(report);
    }
    let mut bodies = None;
    if let Some(placed) = placed {
        let (cuts, meshed) = hollowing::cut(placed, wall)?;
        if talk {
            print!("{cuts}");
        }
        bodies = Some(meshed);
    }
    Ok((hollow, bodies))
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

/// Cuts the staged plate and writes it, or only measures it when there is no panel to
/// draw on.
///
/// A printable file is the engine's run over the whole plate; a PNG stack and a run with
/// no printer are cut here, because neither needs a container and the PNG stack is a
/// debug artefact rather than a printer format.
pub fn slice_staged(
    staged: &mut Staged,
    output: &Path,
    job: &JobArgs,
    watch: &Watch,
) -> Result<(SliceReport, Option<RasterReport>)> {
    let format = format_of(output, job.raster.ctb_version);
    let window = raster_window(&job.raster);
    let plate = format.and_then(|format| staged.take_plate(Some(format), window));
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
    Ok((slice, raster))
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
    let mesh = &baked.mesh;
    let mut slice = report_of(mesh, &windows, staged.drainage);

    let Some(profile) = staged.printer.as_ref() else {
        tracing::warn!("rasterisation needs --profile to know the panel; nothing was written");
        measure(mesh, &windows, &mut slice, stop)?;
        return Ok((slice, None));
    };

    let settings = raster_settings(profile, staged.panel);
    settings
        .validate()
        .context("the panel cannot produce a usable mask")?;
    let report = write_stack(
        mesh,
        &windows,
        &mut slice,
        &settings,
        output,
        &Tolerance::of(material),
        raster_window,
        fold_of(staged.remove_islands, material),
        stop,
    )
    .with_context(|| format!("cannot write the mask stack to {}", output.display()))?;
    Ok((slice, Some(report)))
}

/// An empty report over `windows`, watching the stack for trapped resin when `drainage`
/// asks for it.
pub fn report_of(mesh: &Mesh, windows: &Windows, drainage: bool) -> SliceReport {
    let mut slice = SliceReport::new(windows.settings(), windows.plan().clone());
    if drainage && let Some(bounds) = mesh.aabb() {
        slice.watch_drainage(bounds.mins.truncate(), bounds.maxs.truncate());
    }
    slice
}

/// How the stack folds as it is written: how many bottom layers the plate holds, and
/// whether islands come out.
pub fn fold_of(remove_islands: bool, material: &MaterialProfile) -> Measured {
    let fold = Measured::new(material.bottom_layers as usize);
    if remove_islands {
        fold.removing_islands()
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

/// Presses the file's own texture into the welded mesh, where `--relief` asked for it.
///
/// The map is taken through the weld first: welding is the one step that renumbers the
/// faces it is indexed by.
fn press_relief(
    mesh: Mesh,
    welded: &Welded,
    loaded: &Loaded,
    args: &ImportArgs,
    talk: bool,
) -> Result<Mesh> {
    let Some(amplitude_mm) = args.relief else {
        return Ok(mesh);
    };
    let uvs = loaded
        .uvs
        .as_ref()
        .context("--relief needs a model carrying texture coordinates")?
        .without(&welded.dropped);
    if loaded.textures.is_empty() {
        anyhow::bail!("--relief needs a texture beside the model");
    }
    let heights = loaded
        .textures
        .iter()
        .map(|texture| {
            texture
                .decode()
                .with_context(|| format!("reading {}", texture.name))
        })
        .collect::<Result<Vec<_>>>()?;

    let started = Instant::now();
    let bvh = Bvh::build(&mesh);
    let relief = press(
        &mesh,
        &bvh,
        &uvs,
        &heights,
        &ReliefSettings {
            amplitude_mm,
            precision: args.precision,
            ..ReliefSettings::default()
        },
    )
    .context("pressing the texture into the model")?;

    if talk {
        let lattice = match relief.coarsened {
            true => format!(
                "{:.2} mm lattice, coarsened to fit the memory budget",
                relief.voxel_mm
            ),
            false => format!("{:.2} mm lattice", relief.voxel_mm),
        };
        println!(
            "Relief: {amplitude_mm} mm of {} image(s) pressed in on a {lattice}, {} faces \
             became {}, took {:.1}s\n",
            heights.len(),
            mesh.faces.len(),
            relief.mesh.faces.len(),
            started.elapsed().as_secs_f32()
        );
    }
    Ok(relief.mesh)
}

/// Loads and repairs a model without slicing it, for `inspect`.
pub fn inspect(input: &Path, args: &ImportArgs, chosen: &Chosen) -> Result<ImportReport> {
    let profile = chosen.printer.as_ref();
    let loaded = load(input)?;
    let welded = weld(&loaded.mesh, args.weld_tolerance);
    let (mesh, _) = place(welded.mesh.clone(), args, profile)?;
    // Pressed here too, so that what `inspect` measures is what would be printed.
    let mut mesh = press_relief(mesh, &welded, &loaded, args, false)?;

    let orientation = (!args.no_validate).then(|| orient_outward(&mut mesh));
    let diagnostics = (!args.no_validate).then(|| diagnose(&mesh));
    let stats = MeshStats::of(&mesh).context("mesh has no vertices")?;
    let fit = profile.map(|p| FitCheck::of(&stats, p));

    Ok(ImportReport {
        path: input.to_path_buf(),
        stats,
        welded,
        diagnostics,
        orientation,
        fit,
    })
}

/// What an adaptive run is allowed to do: the layer height becomes its ceiling.
fn adaptive_settings(job: &JobArgs, layer_height: Scalar, talk: bool) -> AdaptiveSettings {
    let settings = AdaptiveSettings {
        cusp_mm: job.slicing.cusp,
        min_height_mm: job.slicing.min_layer_height,
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
fn measure(mesh: &Mesh, windows: &Windows, report: &mut SliceReport, stop: &Stop) -> Result<()> {
    windows
        .stream(mesh, |sliced| {
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

/// Applies the requested orientation, scale, rotation and placement, in that order.
fn place(
    mesh: Mesh,
    args: &ImportArgs,
    profile: Option<&PrinterProfile>,
) -> Result<(Mesh, Option<OrientSummary>)> {
    let transform = &args.transform;
    let (mesh, oriented) = if transform.orient {
        let found = orient(&mesh, &OrientSettings::default())
            .context("cannot find an orientation for this model")?;
        let summary = OrientSummary {
            degrees: found.rotation.to_axis_angle().1.to_degrees(),
            peak_mm2: found.score.peak_mm2.unwrap_or_default(),
            overhang_mm2: found.score.overhang_mm2,
        };
        (
            transform_mesh(
                &mesh,
                Transform {
                    rotation: found.rotation,
                    ..Transform::default()
                },
            ),
            Some(summary),
        )
    } else {
        (mesh, None)
    };

    let placement = Transform {
        translation: Vec3::ZERO,
        rotation: transform.rotate.map_or(Quat::IDENTITY, euler_degrees),
        scale: transform.scale.unwrap_or(Vec3::ONE),
    };

    let mut mesh = if placement == Transform::default() {
        mesh
    } else {
        transform_mesh(&mesh, placement)
    };

    if transform.center {
        let bounds = mesh.aabb().context("mesh has no vertices")?;
        let plate = profile.map_or(Vec3::ZERO, |p| {
            Vec3::new(p.build_volume.x, p.build_volume.y, 0.0)
        });
        let offset = drop_to_plate(&bounds) + center_over_plate(&bounds, plate.x, plate.y);
        mesh = transform_mesh(&mesh, Transform::from_translation(offset));
    }
    Ok((mesh, oriented))
}

fn euler_degrees(angles: Vec3) -> Quat {
    let radians = angles * (std::f32::consts::PI / 180.0);
    Quat::from_rotation_z(radians.z)
        * Quat::from_rotation_y(radians.y)
        * Quat::from_rotation_x(radians.x)
}
