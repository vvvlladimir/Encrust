//! The groups of flags several subcommands share, flattened into each that takes them.

use std::num::NonZeroU8;
use std::path::PathBuf;

use anyhow::{Context, Result};
use core_engine::SupportPlan;
use core_format::ExposureRange;
use core_geometry::{
    DEFAULT_SEAM_TOLERANCE, DEFAULT_SMALLEST_SHELL_MM2, DEFAULT_WELD_TOLERANCE, MendMethod, Scalar,
    Vec3,
};
use core_plate::{OrientGoal, OrientPreset};
use core_slicer::{AdaptiveSettings, WINDOW_LAYERS};
use core_volume::DEFAULT_WRAP_MM;
use core_volume::Tiling;
use printer_profiles::SupportProfile;

use crate::hollowing::HollowArgs;
use crate::modelling::ModelArgs;
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

    /// Skip the topology check, dropping faces laid over others, and the orientation fix.
    #[arg(long)]
    pub no_validate: bool,

    /// Repair the model as the window's Repair does, by `repair` unless another method is
    /// named; it is kept as it came if that would leave more wrong.
    #[arg(long, value_name = "METHOD", num_args = 0..=1, require_equals = true,
          default_missing_value = "repair", conflicts_with = "no_validate")]
    pub repair: Option<RepairMethod>,

    /// How far a vertex may stand off an open edge for --repair to stitch it in, millimetres.
    #[arg(long, value_name = "MM", default_value_t = DEFAULT_SEAM_TOLERANCE, requires = "repair")]
    pub seam_tolerance: Scalar,

    /// Smallest area a shell enclosing nothing keeps under --repair, square millimetres.
    #[arg(long, value_name = "MM2", default_value_t = DEFAULT_SMALLEST_SHELL_MM2,
          requires = "repair")]
    pub smallest_shell: Scalar,

    /// The lattice --repair=wrap falls back to where it cannot cut exactly, millimetres.
    #[arg(long, value_name = "MM", default_value_t = DEFAULT_WRAP_MM, requires = "repair")]
    pub wrap_mm: Scalar,

    /// Turn these faces the other way, numbered from 0 on the model as import leaves it.
    #[arg(
        long,
        value_name = "FACES",
        value_delimiter = ',',
        conflicts_with = "no_validate"
    )]
    pub flip_faces: Vec<usize>,

    /// Drop these faces, numbered as for --flip-faces.
    #[arg(
        long,
        value_name = "FACES",
        value_delimiter = ',',
        conflicts_with = "no_validate"
    )]
    pub drop_faces: Vec<usize>,

    /// Drop these shells, numbered from 1 as `inspect --shells` lists them.
    #[arg(
        long,
        value_name = "SHELLS",
        value_delimiter = ',',
        conflicts_with = "no_validate"
    )]
    pub drop_shells: Vec<usize>,

    /// Lay faces between two open edges, each as its two vertices: A-B:C-D. May repeat.
    #[arg(long, value_name = "A-B:C-D", conflicts_with = "no_validate")]
    pub bridge: Vec<crate::edits::BridgeEdges>,

    #[command(flatten)]
    pub transform: TransformArgs,

    /// Press the file's own texture into the model as relief, this many mm deep; negative sinks it.
    /// Needs a model carrying UVs and an image beside it, or --relief-image.
    #[arg(long, value_name = "MM", allow_hyphen_values = true)]
    pub relief: Option<Scalar>,

    /// Press this picture in instead, tiled over the model from three sides, so a model
    /// with no UVs takes a relief too.
    #[arg(long, value_name = "PATH", requires = "relief")]
    pub relief_image: Option<PathBuf>,

    /// How wide one tile of --relief-image is on the model, millimetres.
    #[arg(long, value_name = "MM", default_value_t = Tiling::default().tile_mm,
          requires = "relief_image")]
    pub relief_tile: Scalar,

    /// How far the tiles of --relief-image are turned, degrees.
    #[arg(
        long,
        value_name = "DEG",
        default_value_t = 0.0,
        allow_hyphen_values = true,
        requires = "relief_image"
    )]
    pub relief_angle: Scalar,

    /// Read the texture's black as high and its white as low.
    #[arg(long, requires = "relief")]
    pub relief_invert: bool,

    /// Read mid grey as the surface itself: white raises it and black sinks it, each by
    /// half of --relief, so the model keeps about its size.
    #[arg(long, requires = "relief")]
    pub relief_symmetric: bool,

    /// Blur the texture by this many of its pixels before pressing it.
    #[arg(long, value_name = "PX", default_value_t = 0.0, requires = "relief")]
    pub relief_smooth: Scalar,

    /// Leave faces within this many degrees of facing the plate as they are.
    #[arg(long, value_name = "DEG", default_value_t = 0.0, requires = "relief")]
    pub relief_hold_bottom: Scalar,

    /// Leave faces within this many degrees of facing straight up as they are.
    #[arg(long, value_name = "DEG", default_value_t = 0.0, requires = "relief")]
    pub relief_hold_top: Scalar,

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

    /// Turn the model the way it prints best, at its --scale and before anything else.
    #[arg(long)]
    pub orient: bool,

    /// What --orient searches for.
    #[arg(long, value_enum, default_value_t = Goal::FewestSupports, requires = "orient")]
    pub orient_goal: Goal,

    /// How far --orient may turn the model from the way its file stands it, degrees.
    #[arg(long, value_name = "DEG", requires = "orient")]
    pub max_tilt: Option<Scalar>,
}

/// What an orientation is searched for, as [`OrientPreset`] names it.
/// Which repair `--repair` runs.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum RepairMethod {
    /// Collapse, stitch seams, drop specks, patch holes and turn the patches outward.
    Repair,
    /// Patch every hole and turn the patches outward.
    CloseHoles,
    /// Patch only the holes a single missing face leaves.
    CloseSmallHoles,
    /// Turn the faces wound the wrong way, and add nothing.
    Turn,
    /// Repair, then wrap every shell in one outer surface.
    Wrap,
    /// Repair, wrap and repair again.
    Deep,
}

impl From<RepairMethod> for core_volume::RepairMethod {
    fn from(method: RepairMethod) -> Self {
        match method {
            RepairMethod::Repair => Self::Mend(MendMethod::Repair),
            RepairMethod::CloseHoles => Self::Mend(MendMethod::CloseHoles),
            RepairMethod::CloseSmallHoles => Self::Mend(MendMethod::CloseSmallHoles),
            RepairMethod::Turn => Self::Mend(MendMethod::Turn),
            RepairMethod::Wrap => Self::Wrap,
            RepairMethod::Deep => Self::Deep,
        }
    }
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Goal {
    /// The least resin in the supports.
    FewestSupports,
    /// The lightest pull on the film.
    LeastPeel,
    /// The shortest stack.
    ShortestPrint,
    /// Every goal at once, each giving up as even a share of its best as it can.
    Auto,
}

impl From<Goal> for OrientPreset {
    fn from(goal: Goal) -> Self {
        match goal {
            Goal::FewestSupports => OrientGoal::FewestSupports.into(),
            Goal::LeastPeel => OrientGoal::LeastPeel.into(),
            Goal::ShortestPrint => OrientGoal::ShortestPrint.into(),
            Goal::Auto => Self::Auto,
        }
    }
}

/// What the model stands on.
#[derive(Debug, clap::Args)]
#[command(next_help_heading = "Supports")]
pub struct SupportArgs {
    /// Stand supports under the model before slicing it, built to a profile of the
    /// catalogue: light, medium, heavy or one saved in the window; `auto` sizes each one by
    /// what it holds, as the window's Auto does.
    #[arg(long, value_name = "ID")]
    pub supports: Option<String>,

    /// Support profile as a TOML file. Wins over --supports.
    #[arg(long, value_name = "PATH")]
    pub support_profile: Option<PathBuf>,

    /// Hold up the cavity --hollow leaves as well as the outside.
    #[arg(long)]
    pub support_cavity: bool,
}

impl SupportArgs {
    /// What this run stands supports to, or `None` for a run that stands none.
    pub fn plan(&self) -> Result<Option<SupportPlan>> {
        let plan = if let Some(path) = &self.support_profile {
            let profile = SupportProfile::load(path)
                .with_context(|| format!("cannot load {}", path.display()))?;
            Some(SupportPlan::of(profile))
        } else {
            self.supports
                .as_deref()
                .map(crate::profiles::plan_of)
                .transpose()?
        };
        Ok(plan.map(|plan| SupportPlan {
            cavity: self.support_cavity,
            ..plan
        }))
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
    /// number of it; defaults to half the layer height, which always is one.
    #[arg(long, value_name = "MM")]
    pub min_layer_height: Option<Scalar>,

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

    /// Take every island out of the written file: a piece cured over nothing, and nothing
    /// that stood on it.
    #[arg(long)]
    pub remove_islands: bool,

    /// With --remove-islands, take this many layers more of whatever is left hanging over
    /// a removed island, one layer of it a pass.
    #[arg(
        long,
        value_name = "N",
        default_value_t = 0,
        requires = "remove_islands"
    )]
    pub hanging_passes: u32,

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
    pub model: ModelArgs,

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
