//! What each tool takes, as the JSON Schema a client is shown, and the checks a value passes
//! before it reaches the command line.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use rmcp::schemars::{self, JsonSchema};
use serde::Deserialize;

use crate::recipe::{Drains, Hollow, Mend, Modelling, Pose, Relief, Repair, Slicing, Supports};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListProfiles {
    /// Keeps only the printers whose id, maker or name holds this, ignoring case.
    pub printer_query: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListRecipes {}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UseRecipe {
    pub job: String,
    /// The recipe the job is prepared by: an id `list_recipes` lists, or a recipe TOML's
    /// absolute path. Absent goes back to the autopilot, `prepare`.
    pub recipe: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SaveRecipe {
    /// What the user calls it; the window and `list_recipes` list it under this name.
    pub name: String,
    /// A job whose steps are kept, as its check ran them with each edit folded in. Not with
    /// `toml`.
    pub job: Option<String>,
    /// The job's revision, counted from one; absent for the latest.
    #[schemars(range(min = 1))]
    pub revision: Option<u32>,
    /// A recipe written from the user's words, in the TOML `list_recipes` shows. Not with
    /// `job`.
    pub toml: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WriteTestPrint {
    pub print: TestPrint,
    /// Absolute path of the folder the STL is written into, under the print's own name.
    pub folder: PathBuf,
}

/// The project's test prints, as the window's start page offers them.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TestPrint {
    /// Spikes, ledges and pillars: the finest detail that prints.
    Detail,
    /// Pins, fins, slots and holes from fine to coarse: whether the resin's exposure is short
    /// or long. Printed with the exposure the maker states, then the user reads which pins
    /// and holes came out.
    Exposure,
    /// Faces at each angle: the finish a surface takes.
    Surface,
    /// A hole and pins at each clearance: what fits.
    Fit,
}

impl TestPrint {
    pub fn name(self) -> &'static str {
        match self {
            Self::Detail => "detail",
            Self::Exposure => "exposure",
            Self::Surface => "surface",
            Self::Fit => "fit",
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ImportResin {
    /// Absolute path of what the user's resin is read from: a sliced file that printed well
    /// with it, another program's exported resin (`.cfgx`, `.ini`), or a resin TOML.
    pub file: PathBuf,
    /// The printer the resin is set up on: an id `list_profiles` lists. Needed where the file
    /// names no machine, or several share its panel.
    pub printer: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateResin {
    /// The printer it is set up on: an id `list_profiles` lists.
    pub printer: String,
    /// What it is called, as on its bottle.
    pub name: String,
    /// Exposure of a normal layer, s, as its maker states it for this printer.
    pub exposure_s: f32,
    /// Exposure of a bottom layer, s.
    pub bottom_exposure_s: f32,
    #[schemars(range(min = 1))]
    pub bottom_layers: u32,
    /// The layer height the numbers are for, mm; 0.05 when absent.
    pub layer_height_mm: Option<f32>,
    pub light_off_delay_s: Option<f32>,
    pub lift_distance_mm: Option<f32>,
    pub lift_speed_mm_min: Option<f32>,
    pub retract_speed_mm_min: Option<f32>,
    /// Layers over which the exposure fades from the bottom one to the normal one.
    pub transition_layers: Option<u16>,
    pub density_g_cm3: Option<f32>,
}

impl CreateResin {
    /// The flags of `profiles new-resin` these numbers are.
    pub fn flags(&self) -> Vec<String> {
        let mut flags = vec![
            format!("--printer={}", self.printer),
            format!("--name={}", self.name),
            format!("--exposure={}", self.exposure_s),
            format!("--bottom-exposure={}", self.bottom_exposure_s),
            format!("--bottom-layers={}", self.bottom_layers),
        ];
        let stated = [
            ("--layer-height", self.layer_height_mm),
            ("--light-off-delay", self.light_off_delay_s),
            ("--lift-distance", self.lift_distance_mm),
            ("--lift-speed", self.lift_speed_mm_min),
            ("--retract-speed", self.retract_speed_mm_min),
            ("--transition-layers", self.transition_layers.map(f32::from)),
            ("--density", self.density_g_cm3),
        ];
        flags.extend(
            stated
                .into_iter()
                .filter_map(|(flag, value)| Some(format!("{flag}={}", value?))),
        );
        flags
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Model {
    /// Absolute path of an STL, OBJ or 3MF file on this computer.
    pub path: PathBuf,
    pub repair: Option<Repair>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Start {
    /// Absolute path of an STL, OBJ or 3MF file on this computer, or of a project (`.encrust`)
    /// the window saved, which is checked and written as it was saved.
    pub model: PathBuf,
    /// The printer the user named: an id `list_profiles` lists, or a printer TOML's path.
    pub printer: String,
    /// The resin the user named: an id `list_profiles` lists, or a resin TOML's path.
    pub resin: String,
    pub repair: Option<Repair>,
    /// Lets the engine's autopilot decide every step no tool sets: repair when broken, the
    /// orientation search, a 2 mm hollow where it saves resin, drains, supports. True when
    /// absent, false for a project; what it decided is in each model's `prepared`.
    pub prepare: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct JobRef {
    /// The id `start_job` returned.
    pub job: String,
    /// Counted from one; absent for the latest.
    #[schemars(range(min = 1))]
    pub revision: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetPose {
    pub job: String,
    /// Absent stands the model as its file does. A new pose clears drain points, not `auto`.
    pub pose: Option<Pose>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ComparePoses {
    pub job: String,
    /// A search to compare the ways of, as `set_pose` takes it; absent, the latest revision's
    /// search, or `auto`. Its `pick` is ignored.
    pub pose: Option<Pose>,
    /// How many of the search's best ways, 2 to 8; 4 when absent.
    #[schemars(range(min = 2, max = 8))]
    pub count: Option<u16>,
    /// Draws each way; true when absent. Each picture is one more run of the command line.
    pub pictures: Option<bool>,
    /// Absent draws `iso`.
    pub views: Option<Vec<View>>,
    /// Each picture's width and height in pixels, 64 to 1024; 320 when absent.
    #[schemars(range(min = 64, max = 1024))]
    pub size_px: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetHollow {
    pub job: String,
    /// Absent leaves the model solid.
    pub hollow: Option<Hollow>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetDrains {
    pub job: String,
    /// Absent drills no hole.
    pub drains: Option<Drains>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetSupports {
    pub job: String,
    /// Absent stands the model on nothing but the plate.
    pub supports: Option<Supports>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetShape {
    pub job: String,
    /// Absent leaves the model's size and shape as its file has them.
    pub modelling: Option<Modelling>,
    /// Absent presses no texture in.
    pub relief: Option<Relief>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetRepair {
    pub job: String,
    /// Absent repairs nothing.
    pub repair: Option<Repair>,
    /// Absent changes no face by hand and keeps repair's defaults.
    pub mend: Option<Mend>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetSlicing {
    pub job: String,
    /// Absent slices and draws by the resin's and the printer's own settings.
    pub slicing: Option<Slicing>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Revert {
    pub job: String,
    /// The revision to make the latest again, counted from one.
    #[schemars(range(min = 1))]
    pub revision: u32,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WriteFile {
    pub job: String,
    /// Counted from one; absent for the latest.
    #[schemars(range(min = 1))]
    pub revision: Option<u32>,
    /// Absolute path of the file to write; its extension picks the format (`.goo`, `.ctb`, …).
    pub output: PathBuf,
    /// Writes even when the check names defects. Only when the user accepted them.
    #[serde(default)]
    pub accept_defects: bool,
}

/// A side the plate is drawn from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum View {
    /// From the front right corner, above: the sliced file's thumbnail.
    Iso,
    Front,
    Back,
    Left,
    Right,
    Top,
    /// From under the plate, X to the right: drain holes and support feet.
    Bottom,
}

impl View {
    pub fn name(self) -> &'static str {
        match self {
            Self::Iso => "iso",
            Self::Front => "front",
            Self::Back => "back",
            Self::Left => "left",
            Self::Right => "right",
            Self::Top => "top",
            Self::Bottom => "bottom",
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Render {
    pub job: String,
    /// Counted from one; absent for the latest.
    #[schemars(range(min = 1))]
    pub revision: Option<u32>,
    /// Absent draws `iso` and `bottom`.
    pub views: Option<Vec<View>>,
    /// Each picture's width and height in pixels, 64 to 1024; 512 when absent.
    #[schemars(range(min = 64, max = 1024))]
    pub size_px: Option<u32>,
    /// Marks where the check found each problem; the answer says which mark is which, by
    /// its pixel. True when absent.
    pub mark_findings: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SendFile {
    /// Absolute path of a file `write_file` wrote.
    pub file: PathBuf,
    /// The printer's IP address or host on the local network.
    pub address: String,
    /// Starts the print once the printer has checked it. Only when the user asked to start
    /// and said the vat is filled and the plate and film are clear.
    #[serde(default)]
    pub start: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReportPrint {
    pub job: String,
    /// The revision printed, counted from one; absent for the one last written, else the
    /// latest.
    #[schemars(range(min = 1))]
    pub revision: Option<u32>,
    /// What the print showed, as the user tells it or as their photo shows it.
    #[schemars(length(min = 1))]
    pub symptoms: Vec<Symptom>,
    /// Where on the plate it showed, plate mm as the revision stands, if the user can say.
    pub at_mm: Option<Vec<[f32; 3]>>,
    /// The user's own words, kept with the print.
    pub note: Option<String>,
}

/// What a person saw on a failed print.
#[derive(Debug, Clone, Copy, Deserialize, serde::Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Symptom {
    /// The print, or a part of it, never stuck to the plate or came off it.
    OffPlate,
    /// The layers came apart, or a line runs round the print.
    LayerSplit,
    /// A hole, a crater or a blowout in a wall.
    Hole,
    /// A support broke or tore out of the model; the part it held is missing or sagged.
    SupportPulled,
    /// Support marks or scars on a face meant to be seen.
    FaceMark,
    /// A hollow print cracked or leaks, often days later.
    ResinCrack,
}

impl Symptom {
    pub fn name(self) -> &'static str {
        match self {
            Self::OffPlate => "off-plate",
            Self::LayerSplit => "layer-split",
            Self::Hole => "hole",
            Self::SupportPulled => "support-pulled",
            Self::FaceMark => "face-mark",
            Self::ResinCrack => "resin-crack",
        }
    }
}

/// A path from the client: absolute, so it means one file whatever the server's directory,
/// and so never read as a flag.
pub fn absolute(path: &Path, what: &str) -> Result<()> {
    if !path.is_absolute() {
        bail!("{what} {} is not an absolute path", path.display());
    }
    Ok(())
}

/// A value from the client that lands on the command line as an argument of its own.
pub fn plain(value: &str, what: &str) -> Result<()> {
    if value.is_empty() || value.starts_with('-') {
        bail!("{what} {value:?} is not a name the command line takes");
    }
    Ok(())
}
