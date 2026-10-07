//! The groups of flags several subcommands share, flattened into each that takes them.

use std::num::NonZeroU8;
use std::path::PathBuf;

use anyhow::{Context, Result};
use core_format::ExposureRange;
use core_geometry::{DEFAULT_WELD_TOLERANCE, Scalar, Vec3};
use core_slicer::{AdaptiveSettings, WINDOW_LAYERS};
use printer_profiles::SupportProfile;

use crate::hollowing::HollowArgs;
use crate::profiles::Selection;
use crate::report::{parse_exposure_band, parse_rotation, parse_scale};
use crate::sliced_file::CtbRevision;

/// Which printer to check against and which resin to expose with.
#[derive(Debug, Default, clap::Args)]
#[command(next_help_heading = "Printer and resin")]
pub struct ProfileArgs {
    /// Printer profile as a TOML file. Wins over --printer.
    #[arg(short, long, value_name = "PATH")]
    pub profile: Option<PathBuf>,

    /// Printer from the catalogue, by id: --printer elegoo-mars-4-ultra.
    #[arg(long, value_name = "ID")]
    pub printer: Option<String>,

    /// Resin profile as a TOML file, holding the exposure and lift settings. Wins over --resin.
    #[arg(short, long, value_name = "PATH")]
    pub material: Option<PathBuf>,

    /// Resin from the catalogue, by id. Its numbers are retuned for the chosen printer.
    #[arg(long, value_name = "ID")]
    pub resin: Option<String>,
}

impl ProfileArgs {
    pub fn selection(&self) -> Selection<'_> {
        Selection {
            printer_id: self.printer.as_deref(),
            printer_path: self.profile.as_deref(),
            resin_id: self.resin.as_deref(),
            resin_path: self.material.as_deref(),
        }
    }
}

/// What happens to a model on its way in: repair, placement and relief.
#[derive(Debug, Clone, clap::Args)]
#[command(next_help_heading = "Model")]
pub struct ImportArgs {
    /// Distance below which vertices are merged, millimetres.
    #[arg(long, value_name = "MM", default_value_t = DEFAULT_WELD_TOLERANCE)]
    pub weld_tolerance: Scalar,

    /// Skip the topology check and the orientation fix.
    #[arg(long)]
    pub no_validate: bool,

    #[command(flatten)]
    pub transform: TransformArgs,

    /// Press the file's own texture into the model as relief, this many millimetres deep.
    /// Negative sinks it in. Needs a model carrying UVs and an image beside it.
    #[arg(long, value_name = "MM", allow_hyphen_values = true)]
    pub relief: Option<Scalar>,

    /// How fine the lattice --relief and --hollow are cut on, 0 to 1. Not the layer height:
    /// the outside of the model never passes through the lattice.
    #[arg(long, value_name = "0..1", default_value_t = 0.5)]
    pub precision: Scalar,
}

/// Where the model stands on the plate.
#[derive(Debug, Clone, clap::Args)]
pub struct TransformArgs {
    /// Rotation in degrees around X, Y and Z, applied in that order.
    #[arg(long, value_name = "X,Y,Z", value_parser = parse_rotation, allow_hyphen_values = true)]
    pub rotate: Option<Vec3>,

    /// Uniform factor, or per-axis factors as X,Y,Z.
    #[arg(long, value_name = "S", value_parser = parse_scale, allow_hyphen_values = true)]
    pub scale: Option<Vec3>,

    /// Centre the model on the plate and sit it on z = 0.
    #[arg(long)]
    pub center: bool,

    /// Turn the model the way it prints best before anything else is done to it.
    #[arg(long)]
    pub orient: bool,
}

/// What the model stands on.
#[derive(Debug, clap::Args)]
#[command(next_help_heading = "Supports")]
pub struct SupportArgs {
    /// Stand supports under the model before slicing it, built to a profile of the
    /// catalogue: light, medium, heavy or one saved in the window.
    #[arg(long, value_name = "ID")]
    pub supports: Option<String>,

    /// Support profile as a TOML file. Wins over --supports.
    #[arg(long, value_name = "PATH")]
    pub support_profile: Option<PathBuf>,
}

impl SupportArgs {
    /// The support profile this run builds to, or `None` for a run that stands none.
    pub fn profile(&self) -> Result<Option<SupportProfile>> {
        if let Some(path) = &self.support_profile {
            return SupportProfile::load(path)
                .with_context(|| format!("cannot load {}", path.display()))
                .map(Some);
        }
        self.supports
            .as_deref()
            .map(crate::profiles::support)
            .transpose()
    }
}

/// How the model is cut into layers, and how long each is exposed.
#[derive(Debug, clap::Args)]
#[command(next_help_heading = "Slicing")]
pub struct SliceArgs {
    /// Layer thickness in millimetres. Defaults to the material profile's own height.
    #[arg(short = 'l', long, value_name = "MM")]
    pub layer_height: Option<Scalar>,

    /// Exposure over a band of print height, repeatable: --exposure-at 0:10:4.5.
    /// The bottom block keeps the resin's own exposure whatever the bands say.
    #[arg(long = "exposure-at", value_name = "FROM:TO:SECONDS", value_parser = parse_exposure_band)]
    pub exposure_at: Vec<ExposureRange>,

    /// Slice thick where the surface is vertical and thin where it is shallow, against
    /// the cusp target of --cusp. --layer-height becomes the thickest layer allowed.
    #[arg(long)]
    pub adaptive: bool,

    /// Stair-step an adaptive stack may leave on the surface, millimetres.
    #[arg(long, value_name = "MM", default_value_t = AdaptiveSettings::default().cusp_mm)]
    pub cusp: Scalar,

    /// Thinnest layer an adaptive stack may use, millimetres. Every thickness is a whole
    /// number of it.
    #[arg(long, value_name = "MM", default_value_t = AdaptiveSettings::default().min_height_mm)]
    pub min_layer_height: Scalar,

    /// Sample this many planes inside each layer, so a feature thinner than a layer is
    /// not lost to which side of the middle it fell on. Costs its own slicing pass each.
    #[arg(long, value_name = "N", default_value = "1")]
    pub samples_per_layer: NonZeroU8,

    /// Layers sliced at once. Fewer holds less of the stack and rebuilds the face index
    /// more often, which trades memory for time.
    #[arg(long, value_name = "N", default_value_t = WINDOW_LAYERS)]
    pub slice_window: usize,

    /// Look for resin that cannot get out, on a model that was hollowed elsewhere. A run
    /// that hollows checks anyway.
    #[arg(long)]
    pub check_drainage: bool,
}

/// How each layer becomes a mask on the panel.
#[derive(Debug, clap::Args)]
#[command(next_help_heading = "Masks")]
pub struct RasterArgs {
    /// Expose each pixel fully or not at all, for panels that do not honour grey.
    #[arg(long)]
    pub no_anti_alias: bool,

    /// Round anti-aliased edges to this many greys, white included. Off keeps all 255.
    #[arg(long, value_name = "N")]
    pub grey_levels: Option<NonZeroU8>,

    /// Dimmest grey to write, overriding what the printer profile claims its panel cures.
    #[arg(long, value_name = "VALUE")]
    pub grey_floor: Option<u8>,

    /// Fade every edge over a box this many pixels either side, before the floor cuts it.
    #[arg(long, value_name = "PX", default_value = "0",
          value_parser = clap::value_parser!(u8).range(0..=8))]
    pub blur: u8,

    /// Take every island out of the written file: a piece cured over nothing, and then
    /// whatever stood only on it, so a floating part goes whole.
    #[arg(long)]
    pub remove_islands: bool,

    /// Layers rasterised at once. Peak memory is this many masks; defaults to the number
    /// of threads.
    #[arg(long, value_name = "N")]
    pub raster_window: Option<usize>,

    /// Which `.ctb` revision to write, when the output is one.
    #[arg(long, value_name = "N", default_value = "4")]
    pub ctb_version: CtbRevision,
}

/// Everything one model's run is told, shared by `slice` and `batch`.
#[derive(Debug, clap::Args)]
pub struct JobArgs {
    #[command(flatten)]
    pub profile: ProfileArgs,

    #[command(flatten)]
    pub import: ImportArgs,

    #[command(flatten)]
    pub hollow: HollowArgs,

    #[command(flatten)]
    pub supports: SupportArgs,

    #[command(flatten)]
    pub slicing: SliceArgs,

    #[command(flatten)]
    pub raster: RasterArgs,

    /// Exit with code 3 when the model has defects or does not fit.
    #[arg(long)]
    pub strict: bool,
}
