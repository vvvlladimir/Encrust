//! The `.encrust` project file: the plate as it stands, written to be opened as it was. Only cheap
//! pure functions are redone in `restore`; see `docs/formats/encrust-project.md`, ADR 0191.

mod blob;
mod restore;

pub use restore::{hollow_of, supports_of};

use std::io::{Read, Seek, Write};
use std::num::NonZeroU8;
use std::ops::Range;
use std::path::Path;
use std::sync::Arc;

use core_format::ExposureRange;
use core_geometry::{Mat3, Mesh, MeshDiagnostics, Orientation, Region, Scalar, Transform, Vec3};
use core_joint::Joints;
use core_slicer::AdaptiveSettings;
use core_supports::{ProjectSettings, SupportPoint, SupportTree};
use core_volume::{
    Blocker, Channel, DrainHole, HoleShape, HollowMode, HollowSettings, InfillSettings,
    PLUG_GAP_MM, PartSettings,
};
use printer_profiles::{AutoProfile, OutputFormat, Raft, SupportProfile};
use serde::{Deserialize, Serialize};

/// What a project file is called, and what an open or save dialog filters on.
pub const EXTENSION: &str = "encrust";

/// The manifest this build writes. Versions 4 and 5 are read as well; any other is refused,
/// lacking built geometry and diagnostics (ADR 0191, 0205, 0261, 0302).
pub const VERSION: u32 = 6;

/// The last version whose lattice took one size for its cells and walls, which this build
/// reads as the cell's.
const ONE_LATTICE_SIZE: u32 = 5;

/// The last version that carried its printer and resin whole rather than by id.
const WHOLE_PROFILES: u32 = 4;

const MANIFEST: &str = "project.json";
const PREVIEW: &str = "preview.png";

/// The axis a copy is flipped on, or a cut plane stands on, in the model's own frame:
/// for a model standing as it was imported, the plate's axis of the same name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    /// Every axis, in the order a picker lists them.
    pub const ALL: [Self; 3] = [Self::X, Self::Y, Self::Z];

    /// The axis's name as the window shows it.
    pub fn label(self) -> &'static str {
        match self {
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
        }
    }
}

/// Which halves of a cut stay on the plate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Keep {
    #[default]
    Both,
    Below,
    Above,
}

impl Keep {
    /// Every choice, in the order a picker lists them.
    pub const ALL: [Self; 3] = [Self::Both, Self::Below, Self::Above];

    /// The choice as the window shows it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Both => "Both",
            Self::Below => "Lower",
            Self::Above => "Upper",
        }
    }
}

/// The grid of copies the Array button lays out.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Array {
    pub columns: u32,
    pub rows: u32,
    /// Space left between one copy and the next, millimetres.
    pub gap_mm: Scalar,
}

impl Default for Array {
    fn default() -> Self {
        Self {
            columns: 2,
            rows: 2,
            gap_mm: 5.0,
        }
    }
}

/// Why a `.encrust` file could not be opened.
#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("cannot read the project file")]
    Io(#[from] std::io::Error),
    #[error("this is not an Encrust project: {0}")]
    NotAProject(String),
    #[error(
        "this project is of format {found}, and this build reads {VERSION}: open the models it was built from and build the plate again"
    )]
    WrongVersion { found: u32 },
    #[error("the project's manifest is damaged: {0}")]
    BadManifest(#[from] serde_json::Error),
    #[error("the model {name} in the project is missing or damaged")]
    BadMesh { name: String },
}

/// What the plate reduces to, for telling a saved one from an edited one.
///
/// Read on demand rather than every frame: it captures the plate, which clones every
/// painted patch. Asked when the window is being closed, and nowhere else.
pub fn digest(manifest: &Manifest) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_vec(manifest)
        .unwrap_or_default()
        .hash(&mut hasher);
    hasher.finish()
}

/// A project as the window holds it: the manifest, and the geometry of each object in
/// the same order as [`Manifest::objects`].
#[derive(Debug)]
pub struct Project {
    pub manifest: Manifest,
    pub models: Vec<ModelMeshes>,
    /// The plate as a PNG, for whatever lists the file before opening it.
    pub preview: Option<Vec<u8>>,
}

/// The geometry one object carries: the mesh it was imported as, and the shell hollowing
/// built from it, which is what prints.
#[derive(Debug, Clone)]
pub struct ModelMeshes {
    pub source: Arc<Mesh>,
    /// The hollowed shell as it stands, or `None` for a model that is still solid.
    pub shell: Option<Arc<Mesh>>,
}

impl ModelMeshes {
    /// What this object prints as: its shell when it has one, the mesh it came in as
    /// otherwise.
    pub fn printed(&self) -> &Arc<Mesh> {
        self.shell.as_ref().unwrap_or(&self.source)
    }
}

/// Everything a project holds but its meshes, as `project.json` stores it.
#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub version: u32,
    /// The catalogue id of the printer, which the file names but does not carry: a plate
    /// opens on a machine of that id, or on the one already in hand (ADR 0261).
    pub printer: Option<String>,
    /// The catalogue id of the resin, held the same way.
    pub resin: Option<String>,
    /// What the plate is cut with, taken up only together with the printer.
    pub slicing: SlicingState,
    pub supports: SupportState,
    pub hollow: HollowState,
    pub drain: DrainState,
    pub cut: CutState,
    pub array: Array,
    /// One name per plate, in the order the plate bar shows them.
    pub plates: Vec<String>,
    pub active_plate: u32,
    pub objects: Vec<ObjectState>,
}

/// How the plate is cut and drawn: layer height, exposures, anti-aliasing, the container.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlicingState {
    pub layer_height_mm: Scalar,
    pub adaptive: Option<AdaptiveSettings>,
    pub exposure: Vec<ExposureRange>,
    /// Planes cut per layer.
    pub samples: NonZeroU8,
    pub anti_alias: bool,
    /// How many greys a mask may hold, or `None` for all 255.
    pub grey_levels: Option<NonZeroU8>,
    pub blur_px: u8,
    pub remove_islands: bool,
    /// Layers more of what stood on an island taken out that go too; absent, and so
    /// none, before ADR 0228.
    #[serde(default)]
    pub hanging_passes: u32,
    pub format: OutputFormat,
}

/// The support tool as it was left: its groups and what the next support is drawn with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SupportState {
    /// One entry per group, group 0 first; see `docs/decisions/0094`.
    pub groups: Vec<Group>,
    pub active: u16,
    pub fill: ProjectSettings,
    pub brush_radius_mm: Scalar,
    pub flood_angle_deg: Scalar,
    /// How far apart a line of supports drawn with shift stands; a file without it reads
    /// the default (ADR 0332).
    #[serde(default = "inline_spacing")]
    pub inline_spacing_mm: Scalar,
    #[serde(default)]
    pub fill_profiles: FillProfiles,
    /// The raft every model without one of its own stands on (ADR 0253). A file without
    /// it reads as the raft of the first group's profile, which is what it used to be.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raft: Option<Raft>,
    /// The automatic profile supports are sized by; `None`, and a file without it, never
    /// had one chosen (ADR 0336).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sizing: Option<AutoSizing>,
}

/// The automatic profile a run sizes supports by, numbers as edited, and whether a run uses
/// it or builds to the group in hand (ADR 0325, 0336).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutoSizing {
    pub on: bool,
    pub profile: AutoProfile,
}

/// Which saved support profile a fill builds its rim and its inside to, by id; `None`, and
/// a file without it, builds to the group in hand (ADR 0333).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FillProfiles {
    #[serde(default)]
    pub rim: Option<String>,
    #[serde(default)]
    pub inside: Option<String>,
}

/// A named set of support settings that supports are drawn with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Group {
    pub name: String,
    pub profile: SupportProfile,
}

/// What the next hollowing is done with. A hollowed model carries its own [`Cavity`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HollowState {
    pub thickness_mm: Scalar,
    pub mode: HollowMode,
    pub precision: Scalar,
    pub infill_on: bool,
    pub infill: InfillSettings,
    pub blocker_mm: Scalar,
    /// Whether a shell opens through the floor it stands on; a file without it reads a
    /// closed one (ADR 0313).
    #[serde(default)]
    pub open_floor: bool,
}

/// What the next drain hole is drilled to. The holes standing carry their own sizes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrainState {
    pub diameter_mm: Scalar,
    pub depth_mm: Scalar,
    pub taper: Scalar,
    /// The next hole's outline and its turn in degrees, and how much narrower than its hole
    /// a plug is drawn, millimetres a side. A file without them reads a round hole and the
    /// default gap (ADR 0313).
    #[serde(default)]
    pub shape: HoleShape,
    #[serde(default)]
    pub turn_deg: Scalar,
    #[serde(default = "plug_gap")]
    pub plug_gap_mm: Scalar,
}

impl DrainState {
    /// A round hole, with the default plug gap.
    pub fn round(diameter_mm: Scalar, depth_mm: Scalar, taper: Scalar) -> Self {
        Self {
            diameter_mm,
            depth_mm,
            taper,
            shape: HoleShape::Round,
            turn_deg: 0.0,
            plug_gap_mm: PLUG_GAP_MM,
        }
    }
}

fn inline_spacing() -> Scalar {
    core_supports::INLINE_SPACING_MM
}

fn plug_gap() -> Scalar {
    PLUG_GAP_MM
}

/// Where the cut tool's plane stands, which halves it keeps and how they are joined.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CutState {
    pub axis: Axis,
    /// Across Z, millimetres above the plate; across X or Y, millimetres from the model's
    /// centre of mass. Named for the Z cut it began as.
    pub height_mm: Scalar,
    pub keep: Keep,
    /// A file without them reads as a plain cut (ADR 0306).
    #[serde(default)]
    pub joints: Joints,
    /// What Split into its parts goes by; a file without it reads the defaults (ADR 0307).
    #[serde(default)]
    pub parts: PartSettings,
}

/// One model on a plate: where it stands, and what was done to it.
#[derive(Debug, Serialize, Deserialize)]
pub struct ObjectState {
    pub name: String,
    pub plate: u32,
    pub transform: Transform,
    pub visible: bool,
    pub summary: Summary,
    pub supports: ObjectSupportState,
    pub hollow: ObjectHollowState,
    /// The Hollow and Drain settings this model keeps instead of the plate's, where it has
    /// its own (ADR 0227). A file without them reads as a model with none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub own_hollow: Option<HollowState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub own_drain: Option<DrainState>,
    /// The support profiles this model is built to instead of its groups', where it has
    /// its own (ADR 0246). A file without them reads as a model with none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub own_supports: Vec<OwnSupportState>,
    /// The raft this model stands on instead of the plate's, where it has its own
    /// (ADR 0253). A file without it reads as a model with none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub own_raft: Option<Raft>,
    /// Which model of the file, by its place among them, this one is a plug for (ADR 0313).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plug_of: Option<u32>,
    /// The faces the Relief brush painted, the only ones a texture is pressed into while
    /// any are (ADR 0318). A file without it reads as a model with none.
    #[serde(default, skip_serializing_if = "Region::is_empty")]
    pub relief_painted: Region,
}

impl ObjectState {
    /// The plate's `table` of group profiles as this model is built to it: its own
    /// profile in place of a group's wherever it keeps one, and under them the raft that
    /// is its own, or `plate_raft`, or failing both the first group's.
    pub fn support_table(
        &self,
        table: &[SupportProfile],
        plate_raft: Option<Raft>,
    ) -> Vec<SupportProfile> {
        let own = self
            .own_supports
            .iter()
            .map(|own| (own.group, &own.profile));
        own_table(table.to_vec(), own, self.own_raft.or(plate_raft))
    }
}

/// `table` with the profiles one model keeps of its `own` put in place of its groups',
/// by group number, and `raft` under them, or the first group's where there is none. Every
/// group stands on that one raft, so it is written into each; see `docs/decisions/0266`.
pub fn own_table<'a>(
    mut table: Vec<SupportProfile>,
    own: impl IntoIterator<Item = (u16, &'a SupportProfile)>,
    raft: Option<Raft>,
) -> Vec<SupportProfile> {
    for (group, profile) in own {
        if let Some(plate) = table.get_mut(group as usize) {
            plate.clone_from(profile);
        }
    }
    if let Some(raft) = raft.or_else(|| table.first().map(|ground| ground.raft)) {
        for profile in &mut table {
            profile.raft = raft;
        }
    }
    table
}

/// One group's profile as one model keeps it: an edit that has not been saved over the
/// profile it was taken from, or another profile altogether.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OwnSupportState {
    pub group: u16,
    pub profile: SupportProfile,
}

/// What mesh repair found at import, kept because the stored mesh is already repaired and
/// loading it would report nothing to do.
#[derive(Debug, Serialize, Deserialize)]
pub struct Summary {
    pub vertices_merged: usize,
    pub faces_removed: usize,
    pub orientation: Orientation,
    pub diagnostics: MeshDiagnostics,
}

/// One model's supports: the points placed, the regions painted, and every tree
/// standing — the ones an automatic run grew as well as the ones a hand froze.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ObjectSupportState {
    pub points: Vec<SupportPoint>,
    pub painted: Region,
    pub blocked: Region,
    pub frozen: Vec<SupportTree>,
    /// The grown trees, in the model's own space, as the run left them. They are not
    /// grown again on opening: a tree is where the file says it is.
    pub grown: Vec<SupportTree>,
    /// How much of the height in `transform` is the raft's lift, millimetres (ADR 0238).
    /// A file without it reads as a model no raft has lifted.
    #[serde(default)]
    pub raft_lift_mm: f32,
}

/// One model's hollowing: the blockers, drains and channels placed on it, and the shell
/// standing on it.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ObjectHollowState {
    pub blockers: Vec<Blocker>,
    pub drains: Vec<DrainHole>,
    pub channels: Vec<Channel>,
    /// The shell as it was built, or `None` for a model that is still solid. Its geometry
    /// is the object's `shell.mesh`.
    pub built: Option<BuiltCavity>,
}

/// A shell as the run left it: the wall it was asked for, and what came out of the
/// lattice. Everything here is measured, not asked for again, so the file opens as the
/// plate that was saved (ADR 0191).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuiltCavity {
    pub wall: Cavity,
    /// Which faces of the shell bound the space the resin fills.
    pub cavity_faces: Range<usize>,
    pub cavity_mm3: Scalar,
    /// The lattice the cavity came out on, millimetres, and whether a memory budget made
    /// it coarser than the precision asked for.
    pub voxel_mm: Scalar,
    pub coarsened: bool,
    /// The size the model stood at, which the wall was measured under; see [`Self::shape`].
    pub scale: Vec3,
    /// That size whole, written only where the model was stretched along the plate too.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stretched: Option<Mat3>,
}

impl BuiltCavity {
    /// What the wall was measured under (`Transform::unturned`), however the file wrote it.
    pub fn shape(&self) -> Mat3 {
        self.stretched
            .unwrap_or_else(|| Mat3::from_diagonal(self.scale.abs()))
    }

    /// `shape` as the two keys write it: its diagonal, and itself where that is not all.
    pub fn keys_of(shape: Mat3) -> (Vec3, Option<Mat3>) {
        let diagonal = Vec3::new(shape.x_axis.x, shape.y_axis.y, shape.z_axis.z);
        let stretched = (shape != Mat3::from_diagonal(diagonal)).then_some(shape);
        (diagonal, stretched)
    }
}

/// The wall a model was hollowed to, as the Hollow panel shows it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Cavity {
    pub thickness_mm: Scalar,
    pub mode: HollowMode,
    pub precision: Scalar,
    pub infill: Option<InfillSettings>,
    /// The way to the plate the floor was opened toward; absent for a closed shell, and in
    /// a file from before ADR 0313.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_toward: Option<Vec3>,
}

impl Cavity {
    /// The wall `asked` builds, without what the model laid over it.
    pub fn of(asked: &HollowSettings) -> Self {
        Self {
            thickness_mm: asked.thickness_mm,
            mode: asked.mode,
            precision: asked.precision,
            infill: asked.infill,
            open_toward: asked.open_toward,
        }
    }

    /// What a run is asked for to build this wall again, before the model lays its own
    /// blockers and channels over it.
    pub fn settings(&self) -> HollowSettings {
        HollowSettings {
            thickness_mm: self.thickness_mm,
            mode: self.mode,
            precision: self.precision,
            infill: self.infill,
            open_toward: self.open_toward,
            ..HollowSettings::default()
        }
    }
}

/// Writes the project into `sink`, which is the only thing a browser can offer.
pub fn write_to<W: Write + Seek>(sink: W, project: &Project) -> Result<(), ProjectError> {
    let mut zip = zip::ZipWriter::new(sink);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    zip.start_file(MANIFEST, options)?;
    zip.write_all(&serde_json::to_vec_pretty(&project.manifest)?)?;
    if let Some(png) = &project.preview {
        // A PNG is compressed already; deflating it again only costs time.
        let stored = options.compression_method(zip::CompressionMethod::Stored);
        zip.start_file(PREVIEW, stored)?;
        zip.write_all(png)?;
    }
    for (index, model) in project.models.iter().enumerate() {
        zip.start_file(source_name(index), options)?;
        zip.write_all(&blob::write(&model.source))?;
        if let Some(shell) = &model.shell {
            zip.start_file(shell_name(index), options)?;
            zip.write_all(&blob::write(shell))?;
        }
    }
    zip.finish()?;
    Ok(())
}

/// Writes the project to `path`, replacing whatever was there.
pub fn save(path: &Path, project: &Project) -> Result<(), ProjectError> {
    let file = std::fs::File::create(path)?;
    write_to(std::io::BufWriter::new(file), project)
}

/// Reads a project back from `source`. A file of another format version is refused
/// whole: reading as far as the manifest parses would drop what it does not have.
pub fn read_from<R: Read + Seek>(source: R) -> Result<Project, ProjectError> {
    let mut zip = zip::ZipArchive::new(source)
        .map_err(|error| ProjectError::NotAProject(error.to_string()))?;

    let manifest: Manifest = {
        let mut entry = zip
            .by_name(MANIFEST)
            .map_err(|_| ProjectError::NotAProject(format!("no {MANIFEST} inside")))?;
        let mut text = String::new();
        entry.read_to_string(&mut text)?;
        let version: Version = serde_json::from_str(&text)?;
        match version.version {
            VERSION => serde_json::from_str(&text)?,
            ONE_LATTICE_SIZE => Manifest {
                version: VERSION,
                ..serde_json::from_str(&text)?
            },
            WHOLE_PROFILES => by_ids(serde_json::from_str(&text)?)?,
            found => return Err(ProjectError::WrongVersion { found }),
        }
    };

    let mut models = Vec::with_capacity(manifest.objects.len());
    for (index, object) in manifest.objects.iter().enumerate() {
        let source = Arc::new(mesh_at(&mut zip, &source_name(index))?);
        let shell = match object.hollow.built {
            Some(_) => Some(Arc::new(mesh_at(&mut zip, &shell_name(index))?)),
            None => None,
        };
        models.push(ModelMeshes { source, shell });
    }

    let preview = match zip.by_name(PREVIEW) {
        Ok(mut entry) => {
            let mut png = Vec::new();
            entry.read_to_end(&mut png)?;
            Some(png)
        }
        Err(_) => None,
    };
    Ok(Project {
        manifest,
        models,
        preview,
    })
}

/// A version 4 manifest as this build's: the printer and resin it carried whole are kept
/// by their ids alone, and one that had none is not named.
fn by_ids(mut manifest: serde_json::Value) -> Result<Manifest, ProjectError> {
    for key in ["printer", "resin"] {
        if let Some(chosen) = manifest.get_mut(key) {
            *chosen = chosen.get("id").cloned().unwrap_or_default();
        }
    }
    manifest["version"] = VERSION.into();
    Ok(serde_json::from_value(manifest)?)
}

/// Reads a project back from a file at `path`.
pub fn load(path: &Path) -> Result<Project, ProjectError> {
    let file = std::fs::File::open(path)?;
    read_from(std::io::BufReader::new(file))
}

/// The version alone, read before the rest so that a manifest of another format is
/// refused by its number rather than by a field it happens to have renamed.
#[derive(Deserialize)]
struct Version {
    version: u32,
}

/// One mesh blob out of the archive, by the name the manifest implies it has.
fn mesh_at<R: Read + Seek>(zip: &mut zip::ZipArchive<R>, name: &str) -> Result<Mesh, ProjectError> {
    let mut entry = zip.by_name(name).map_err(|_| ProjectError::BadMesh {
        name: name.to_owned(),
    })?;
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes)?;
    blob::read(name, &bytes)
}

fn source_name(index: usize) -> String {
    format!("models/{index}/source.mesh")
}

fn shell_name(index: usize) -> String {
    format!("models/{index}/shell.mesh")
}

impl From<zip::result::ZipError> for ProjectError {
    fn from(error: zip::result::ZipError) -> Self {
        match error {
            zip::result::ZipError::Io(io) => Self::Io(io),
            other => Self::NotAProject(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests;
