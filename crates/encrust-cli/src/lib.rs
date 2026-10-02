//! The `slice` command line: what every flag means, and the run it drives.
//!
//! `main.rs` is argument parsing and an exit code; everything else is here so that a test
//! can drive a run without spawning a process. Worked examples: `docs/cli.md`.

mod batch;
mod hollowing;
mod png_stack;
mod raster_report;
mod slice_report;
mod sliced_file;
mod sliced_read;
mod slicing;
mod stack;
mod stats;
mod supports;

pub mod pipeline;
pub mod profiles;
pub mod report;

use std::num::NonZeroU8;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use core_format::ExposureRange;
use core_geometry::{DEFAULT_WELD_TOLERANCE, Scalar, Vec3};
use core_slicer::AdaptiveSettings;
use printer_profiles::SupportProfile;

use crate::hollowing::HollowArgs;
use crate::report::{parse_exposure_band, parse_rotation, parse_scale};
use crate::sliced_file::CtbRevision;
use crate::slicing::SLICE_WINDOW_LAYERS;

/// Slice a mesh for an MSLA resin printer.
#[derive(Parser, Debug)]
#[command(name = "slice", version, about)]
pub struct Args {
    /// Mesh to slice, or a directory of them for a batch run.
    #[arg(required_unless_present_any = ["list_profiles", "read"])]
    input: Option<PathBuf>,

    /// Where the sliced output goes: a `.goo` or `.ctb` file, or a directory for a PNG
    /// stack.
    #[arg(short, long, default_value = "out")]
    output: PathBuf,

    /// Layer thickness in millimetres. Defaults to the material profile's own height.
    #[arg(short = 'l', long, value_name = "MM")]
    layer_height: Option<Scalar>,

    /// Exposure over a band of print height, repeatable: --exposure-at 0:10:4.5.
    /// The bottom block keeps the resin's own exposure whatever the bands say.
    #[arg(long = "exposure-at", value_name = "FROM:TO:SECONDS", value_parser = parse_exposure_band)]
    exposure_at: Vec<ExposureRange>,

    /// Slice thick where the surface is vertical and thin where it is shallow, against
    /// the cusp target of --cusp. --layer-height becomes the thickest layer allowed.
    #[arg(long)]
    adaptive: bool,

    /// Stair-step an adaptive stack may leave on the surface, millimetres.
    #[arg(long, value_name = "MM", default_value_t = AdaptiveSettings::default().cusp_mm)]
    cusp: Scalar,

    /// Thinnest layer an adaptive stack may use, millimetres. Every thickness is a whole
    /// number of it.
    #[arg(long, value_name = "MM", default_value_t = AdaptiveSettings::default().min_height_mm)]
    min_layer_height: Scalar,

    /// Which `.ctb` revision to write, when the output is one.
    #[arg(long, value_name = "N", default_value = "4")]
    ctb_version: CtbRevision,

    /// Stand supports under the model before slicing it, built to a shipped preset or,
    /// with --support-profile, to a profile of your own.
    #[arg(long, value_name = "PRESET")]
    supports: Option<SupportPreset>,

    /// Support profile as a TOML file. Wins over --supports.
    #[arg(long, value_name = "PATH")]
    support_profile: Option<PathBuf>,

    /// Models cut at once in a batch run. Each one already uses every core, so more of
    /// them at once buys throughput on small models and costs peak memory.
    #[arg(long, value_name = "N", default_value_t = 1)]
    jobs: usize,

    /// Stop after the import report, without slicing.
    #[arg(long)]
    no_slice: bool,

    /// Stop after the slice report, without writing the mask stack.
    #[arg(long)]
    no_raster: bool,

    /// Look for resin that cannot get out, on a model that was hollowed elsewhere. A run
    /// that hollows checks anyway.
    #[arg(long)]
    check_drainage: bool,

    /// Take every island out of the written file: a piece cured over nothing, and then
    /// whatever stood only on it, so a floating part goes whole.
    #[arg(long)]
    remove_islands: bool,

    /// Expose each pixel fully or not at all, for panels that do not honour grey.
    #[arg(long)]
    no_anti_alias: bool,

    /// Sample this many planes inside each layer, so a feature thinner than a layer is
    /// not lost to which side of the middle it fell on. Costs its own slicing pass each.
    #[arg(long, value_name = "N", default_value = "1")]
    samples_per_layer: NonZeroU8,

    /// Round anti-aliased edges to this many greys, white included. Off keeps all 255.
    #[arg(long, value_name = "N")]
    grey_levels: Option<NonZeroU8>,

    /// Dimmest grey to write, overriding what the printer profile claims its panel cures.
    #[arg(long, value_name = "VALUE")]
    grey_floor: Option<u8>,

    /// Fade every edge over a box this many pixels either side, before the floor cuts it.
    #[arg(long, value_name = "PX", default_value = "0",
          value_parser = clap::value_parser!(u8).range(0..=8))]
    blur: u8,

    /// Layers rasterised at once. Peak memory is this many masks; defaults to the number
    /// of threads.
    #[arg(long, value_name = "N")]
    raster_window: Option<usize>,

    /// Layers sliced at once. Fewer holds less of the stack and rebuilds the face index
    /// more often, which trades memory for time.
    #[arg(long, value_name = "N", default_value_t = SLICE_WINDOW_LAYERS)]
    slice_window: usize,

    /// Printer profile to check the model against. Wins over --printer.
    #[arg(short, long)]
    profile: Option<PathBuf>,

    /// Printer from the catalogue, by id: --printer elegoo-mars-4-ultra.
    #[arg(long, value_name = "ID")]
    printer: Option<String>,

    /// Resin profile holding the exposure and lift settings. Wins over --resin.
    #[arg(short, long)]
    material: Option<PathBuf>,

    /// Resin from the catalogue, by id. Its numbers are retuned for the chosen printer.
    #[arg(long, value_name = "ID")]
    resin: Option<String>,

    /// List the printers and resins in the catalogue, then exit.
    #[arg(long)]
    list_profiles: bool,

    /// Open a sliced file, print what it says and decode every layer, then exit. Any
    /// container this writes, whoever wrote the file.
    #[arg(long, value_name = "FILE")]
    read: Option<PathBuf>,

    /// Rotation in degrees around X, Y and Z, applied in that order.
    #[arg(long, value_name = "X,Y,Z", value_parser = parse_rotation)]
    rotate: Option<Vec3>,

    /// Uniform factor, or per-axis factors as X,Y,Z.
    #[arg(long, value_name = "S", value_parser = parse_scale)]
    scale: Option<Vec3>,

    /// Centre the model on the plate and sit it on z = 0.
    #[arg(long)]
    center: bool,

    /// Turn the model the way it prints best before anything else is done to it.
    #[arg(long)]
    orient: bool,

    /// Distance below which vertices are merged, millimetres.
    #[arg(long, value_name = "MM", default_value_t = DEFAULT_WELD_TOLERANCE)]
    weld_tolerance: Scalar,

    /// Skip the topology check and the orientation fix.
    #[arg(long)]
    no_validate: bool,

    /// Press the file's own texture into the model as relief, this many millimetres deep.
    /// Negative sinks it in. Needs a model carrying UVs and an image beside it, and cuts
    /// the relief on the lattice --precision asks for.
    #[arg(long, value_name = "MM")]
    relief: Option<Scalar>,

    #[command(flatten)]
    hollow: HollowArgs,

    /// Exit non-zero when the model has defects or does not fit.
    #[arg(long)]
    strict: bool,

    /// Log level: error, warn, info, debug or trace.
    #[arg(long, default_value = "info")]
    log: String,
}

/// A shipped support preset, for a run that has no profile of its own to point at.
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum SupportPreset {
    Light,
    Medium,
    Heavy,
}

impl Args {
    /// The support profile this run builds to, or `None` for a run that stands none.
    fn support_profile(&self) -> Result<Option<SupportProfile>> {
        if let Some(path) = &self.support_profile {
            return SupportProfile::load(path)
                .with_context(|| format!("cannot load {}", path.display()))
                .map(Some);
        }
        Ok(self.supports.map(|preset| match preset {
            SupportPreset::Light => SupportProfile::light(),
            SupportPreset::Medium => SupportProfile::medium(),
            SupportPreset::Heavy => SupportProfile::heavy(),
        }))
    }
}

/// Sets the log level from `--log`, unless `RUST_LOG` already says otherwise.
pub fn init_tracing(args: &Args) {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&args.log)),
        )
        .init();
}

/// Runs one invocation. Returns false when `--strict` was given and the model is not
/// clean, so the caller can pick an exit code.
pub fn run(args: &Args) -> Result<bool> {
    if args.list_profiles {
        profiles::list()?;
        return Ok(true);
    }
    if let Some(path) = args.read.as_deref() {
        print!("{}", sliced_read::read(path)?);
        return Ok(true);
    }
    let input = args
        .input
        .clone()
        .context("no mesh to slice; clap requires one unless --list-profiles or --read is given")?;

    let chosen = profiles::resolve(&profiles::Selection {
        printer_id: args.printer.as_deref(),
        printer_path: args.profile.as_deref(),
        resin_id: args.resin.as_deref(),
        resin_path: args.material.as_deref(),
    })?;

    if input.is_dir() {
        return batch::run(&input, args, &chosen);
    }

    if args.no_slice {
        let report = pipeline::inspect(&input, args, &chosen)?;
        print!("{report}");
        return Ok(!args.strict || report.is_clean());
    }

    let outcome = pipeline::slice_one(&input, &args.output, args, &chosen, true)?;
    Ok(!args.strict || outcome.is_clean())
}
