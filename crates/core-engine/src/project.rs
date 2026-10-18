//! The `.encrust` project file: the plate as it stands, written to be opened as it was.
//!
//! A zip of one JSON manifest, the meshes of every model — what was imported, and the
//! shell the tools built from it — and a picture of the plate. Nothing the file holds is decided again on opening; only
//! what is a cheap pure function of it is worked out, and `restore` is where that is done.
//! See `docs/formats/encrust-project.md` and `docs/decisions/0191`.

mod blob;
mod restore;

pub use restore::{hollow_of, supports_of};

use std::io::{Read, Seek, Write};
use std::num::NonZeroU8;
use std::ops::Range;
use std::path::Path;
use std::sync::Arc;

use core_format::ExposureRange;
use core_geometry::{Mat3, Mesh, MeshDiagnostics, Orientation, Scalar, Transform, Vec3};
use core_joint::Joints;
use core_slicer::AdaptiveSettings;
use core_supports::{ProjectSettings, Region, SupportPoint, SupportTree};
use core_volume::{
    Blocker, Channel, DrainHole, HollowMode, HollowSettings, InfillSettings, PartSettings,
};
use printer_profiles::{OutputFormat, Raft, SupportProfile};
use serde::{Deserialize, Serialize};

/// What a project file is called, and what an open or save dialog filters on.
pub const EXTENSION: &str = "encrust";

/// The manifest this build writes. Version 4 is read as well, by the ids of its profiles
/// alone; any other is refused, lacking built geometry and diagnostics (ADR 0191, 0205, 0261).
pub const VERSION: u32 = 5;

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
    /// The raft every model without one of its own stands on (ADR 0253). A file without
    /// it reads as the raft of the first group's profile, which is what it used to be.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raft: Option<Raft>,
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
}

/// What the next drain hole is drilled to. The holes standing carry their own sizes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrainState {
    pub diameter_mm: Scalar,
    pub depth_mm: Scalar,
    pub taper: Scalar,
}

/// Where the cut tool's plane stands, which halves it keeps and how they are joined.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CutState {
    pub axis: Axis,
    /// Across Z, millimetres above the plate; across X or Y, millimetres from the model's
    /// centre of mass. Named for the Z cut it began as.
    pub height_mm: Scalar,
    pub keep: Keep,
    /// A file without them reads as a plain cut (ADR 0305).
    #[serde(default)]
    pub joints: Joints,
    /// What Split into its parts goes by; a file without it reads the defaults (ADR 0306).
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
}

impl Cavity {
    /// The wall `asked` builds, without what the model laid over it.
    pub fn of(asked: &HollowSettings) -> Self {
        Self {
            thickness_mm: asked.thickness_mm,
            mode: asked.mode,
            precision: asked.precision,
            infill: asked.infill,
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
mod tests {
    use std::io::Cursor;

    use super::*;
    use core_geometry::Vec3;

    /// A plate of `objects` models, each hollowed, so every part of the format is in it:
    /// a manifest, a source mesh and a shell beside it.
    fn project(objects: usize) -> Project {
        let mesh = Mesh::new(vec![Vec3::ZERO, Vec3::X, Vec3::Y], vec![[0, 1, 2]]);
        Project {
            manifest: Manifest {
                version: VERSION,
                printer: Some("elegoo-saturn-4-ultra".to_owned()),
                resin: None,
                slicing: SlicingState {
                    layer_height_mm: 0.05,
                    adaptive: Some(AdaptiveSettings::default()),
                    exposure: vec![ExposureRange::new(0.0, 4.0, 9.5)],
                    samples: core_slicer::ONE_SAMPLE,
                    anti_alias: true,
                    grey_levels: None,
                    blur_px: 0,
                    remove_islands: false,
                    hanging_passes: 0,
                    format: OutputFormat::Ctb5,
                },
                supports: SupportState {
                    groups: vec![Group {
                        name: "Default".to_owned(),
                        profile: SupportProfile::default(),
                    }],
                    active: 0,
                    fill: ProjectSettings::default(),
                    brush_radius_mm: 3.0,
                    flood_angle_deg: 30.0,
                    raft: None,
                },
                hollow: HollowState {
                    thickness_mm: 2.0,
                    mode: HollowMode::External,
                    precision: 0.5,
                    infill_on: true,
                    infill: InfillSettings::default(),
                    blocker_mm: 2.0,
                },
                drain: DrainState {
                    diameter_mm: 3.0,
                    depth_mm: 3.0,
                    taper: 1.0,
                },
                cut: CutState {
                    axis: Axis::Z,
                    height_mm: 10.0,
                    keep: Keep::Both,
                    joints: core_joint::Joints::default(),
                    parts: PartSettings::default(),
                },
                array: Array::default(),
                plates: vec!["Plate 1".to_owned()],
                active_plate: 0,
                objects: (0..objects).map(object).collect(),
            },
            models: (0..objects)
                .map(|_| ModelMeshes {
                    source: Arc::new(mesh.clone()),
                    shell: Some(Arc::new(mesh.clone())),
                })
                .collect(),
            preview: Some(b"a picture of the plate".to_vec()),
        }
    }

    /// One model on the plate, with a point, a painted face, a blocker, a channel and a
    /// built cavity, so every part of an object's state is written and read back.
    fn object(index: usize) -> ObjectState {
        ObjectState {
            name: format!("model {index}"),
            plate: 0,
            transform: if index == 1 {
                Transform::from_translation(Vec3::new(1.0, 2.0, 3.0))
                    .turned(core_geometry::Quat::from_rotation_x(0.6))
                    .stretched(Vec3::new(1.0, 1.0, 1.5))
            } else {
                Transform::from_translation(Vec3::new(1.0, 2.0, 3.0))
            },
            visible: true,
            summary: Summary {
                vertices_merged: 7,
                faces_removed: 1,
                orientation: Orientation {
                    flipped_faces: 2,
                    inverted_shells: 0,
                    orientable: true,
                },
                diagnostics: MeshDiagnostics {
                    vertices: 3,
                    faces: 1,
                    degenerate_faces: 0,
                    duplicate_faces: 0,
                    unreferenced_vertices: 0,
                    boundary_edges: 3,
                    non_manifold_edges: 0,
                    unbalanced_edges: 3,
                    shells: 1,
                    euler_characteristic: 1,
                },
            },
            supports: ObjectSupportState {
                points: vec![SupportPoint::new(Vec3::Z).in_group(1)],
                painted: marked(&[0]),
                blocked: Region::default(),
                frozen: Vec::new(),
                grown: Vec::new(),
                raft_lift_mm: 0.5,
            },
            hollow: ObjectHollowState {
                blockers: vec![Blocker::ball(Vec3::ZERO, 1.5)],
                drains: Vec::new(),
                channels: vec![Channel {
                    points: vec![Vec3::ZERO, Vec3::Z],
                    diameter_mm: 3.0,
                }],
                built: Some(BuiltCavity {
                    wall: Cavity {
                        thickness_mm: 1.5,
                        mode: HollowMode::External,
                        precision: 0.25,
                        infill: None,
                    },
                    cavity_faces: 0..1,
                    cavity_mm3: 12.5,
                    voxel_mm: 0.2,
                    coarsened: false,
                    scale: Vec3::ONE,
                    stretched: (index == 1).then_some(Mat3::from_cols(
                        Vec3::new(2.0, 0.0, 0.0),
                        Vec3::new(0.5, 1.0, 0.0),
                        Vec3::new(0.0, 0.25, 1.5),
                    )),
                }),
            },
            own_hollow: None,
            own_drain: (index == 1).then_some(DrainState {
                diameter_mm: 4.0,
                depth_mm: 3.0,
                taper: 1.0,
            }),
            own_supports: if index == 1 {
                vec![OwnSupportState {
                    group: 1,
                    profile: SupportProfile::heavy(),
                }]
            } else {
                Vec::new()
            },
            own_raft: None,
        }
    }

    fn marked(faces: &[usize]) -> Region {
        let mut region = Region::default();
        for face in faces {
            region.set(*face, true);
        }
        region
    }

    fn round_trip(project: &Project) -> Project {
        let mut bytes = Cursor::new(Vec::new());
        write_to(&mut bytes, project).expect("a project in memory writes");
        bytes.set_position(0);
        read_from(bytes).expect("what was just written reads")
    }

    #[test]
    fn a_plate_comes_back_as_it_was_saved() {
        let saved = project(2);
        let back = round_trip(&saved);

        assert_eq!(back.manifest.objects.len(), 2);
        assert_eq!(back.models[0].source, saved.models[0].source);
        let object = &back.manifest.objects[0];
        assert_eq!(object.transform, saved.manifest.objects[0].transform);
        assert_eq!(
            object.supports.points,
            saved.manifest.objects[0].supports.points
        );
        assert_eq!(
            object.supports.painted,
            saved.manifest.objects[0].supports.painted
        );
        assert_eq!(
            object.hollow.channels,
            saved.manifest.objects[0].hollow.channels
        );
        assert_eq!(back.manifest.slicing.format, OutputFormat::Ctb5);
        assert_eq!(back.manifest.hollow.mode, HollowMode::External);
    }

    #[test]
    fn a_hollowed_model_brings_its_shell_back_with_what_it_was_measured_at() {
        let saved = project(1);
        let back = round_trip(&saved);

        assert_eq!(back.models[0].shell, saved.models[0].shell);
        assert_eq!(
            back.manifest.objects[0].hollow.built,
            saved.manifest.objects[0].hollow.built
        );
    }

    #[test]
    fn the_cut_joints_and_parts_come_back_and_a_file_without_them_reads_the_defaults() {
        let mut saved = project(1);
        saved.manifest.cut.joints.kind = Some(core_joint::JointKind::Magnets);
        saved.manifest.cut.joints.magnets.thickness_mm = 3.0;
        saved.manifest.cut.parts.smallest_mm3 = 5.0;
        let back = round_trip(&saved);
        assert_eq!(back.manifest.cut.joints, saved.manifest.cut.joints);
        assert_eq!(back.manifest.cut.parts, saved.manifest.cut.parts);

        let older: CutState =
            serde_json::from_str(r#"{ "axis": "z", "height_mm": 10.0, "keep": "both" }"#)
                .expect("a cut written before the joints reads");
        assert_eq!(older.joints.kind, None);
        assert_eq!(older.parts, PartSettings::default());
    }

    #[test]
    fn a_solid_model_carries_one_mesh() {
        let mut saved = project(1);
        saved.manifest.objects[0].hollow.built = None;
        saved.models[0].shell = None;

        let back = round_trip(&saved);
        assert!(back.models[0].shell.is_none());
        assert_eq!(back.models[0].printed(), &back.models[0].source);
    }

    #[test]
    fn a_models_own_settings_come_back_and_a_file_without_them_reads() {
        let back = round_trip(&project(2));
        assert_eq!(back.manifest.objects[0].own_drain, None);
        assert_eq!(
            back.manifest.objects[1]
                .own_drain
                .as_ref()
                .map(|own| own.diameter_mm),
            Some(4.0)
        );
        let mut older = serde_json::to_value(&back.manifest.objects[1]).expect("serialises");
        let fields = older.as_object_mut().expect("an object");
        fields.remove("own_drain");
        let read: ObjectState = serde_json::from_value(older).expect("a file from before reads");
        assert_eq!(read.own_drain, None);
    }

    #[test]
    fn a_model_is_built_to_its_own_support_profile_where_it_keeps_one() {
        let plate = [SupportProfile::light(), SupportProfile::medium()];
        assert_eq!(object(0).support_table(&plate, None), plate.to_vec());
        let heavy = SupportProfile {
            raft: plate[0].raft,
            ..SupportProfile::heavy()
        };
        assert_eq!(
            object(1).support_table(&plate, None),
            vec![SupportProfile::light(), heavy],
            "the second model keeps its own profile of group 1, on the first group's raft"
        );
    }

    #[test]
    fn a_model_stands_on_its_own_raft_then_the_plates_then_the_first_groups() {
        let plate = [SupportProfile::light(), SupportProfile::medium()];
        let thick = Raft {
            enabled: true,
            thickness_mm: 3.5,
            ..plate[0].raft
        };
        let thicker = Raft {
            thickness_mm: 4.5,
            ..thick
        };

        assert_eq!(object(0).support_table(&plate, None)[0].raft, plate[0].raft);
        assert_eq!(object(0).support_table(&plate, Some(thick))[0].raft, thick);
        let mut own = object(0);
        own.own_raft = Some(thicker);
        assert_eq!(own.support_table(&plate, Some(thick))[0].raft, thicker);
        assert_eq!(
            own.support_table(&plate, Some(thick))[1].raft,
            thicker,
            "every group stands on the one raft"
        );
        let mut second = plate.clone();
        second[1].raft = thick;
        assert_eq!(
            object(0).support_table(&second, None)[1].raft,
            plate[0].raft,
            "with none given, every group takes the first group's"
        );
    }

    #[test]
    fn a_models_own_support_profile_comes_back_and_a_file_without_one_reads() {
        let back = round_trip(&project(2));
        assert!(back.manifest.objects[0].own_supports.is_empty());
        assert_eq!(
            back.manifest.objects[1].own_supports,
            vec![OwnSupportState {
                group: 1,
                profile: SupportProfile::heavy(),
            }]
        );
        let mut older = serde_json::to_value(&back.manifest.objects[1]).expect("serialises");
        let fields = older.as_object_mut().expect("an object");
        fields.remove("own_supports");
        let read: ObjectState = serde_json::from_value(older).expect("a file from before reads");
        assert!(read.own_supports.is_empty());
    }

    #[test]
    fn a_stretched_model_comes_back_stretched_and_a_file_from_before_reads() {
        let saved = project(2);
        let back = round_trip(&saved);
        let (object, was) = (&back.manifest.objects[1], &saved.manifest.objects[1]);
        assert_eq!(object.transform, was.transform);
        assert_eq!(object.hollow.built, was.hollow.built);

        let mut older = serde_json::to_value(&back.manifest.objects[0]).expect("serialises");
        assert!(
            older["transform"].get("stretch").is_none(),
            "nothing stretched writes the transform as a file from before has it"
        );
        let built = older["hollow"]["built"].as_object_mut().expect("a shell");
        assert!(built.remove("stretched").is_none());
        built.insert("scale".to_owned(), serde_json::json!([-2.0, 1.0, 0.5]));
        let read: ObjectState = serde_json::from_value(older).expect("a file from before reads");
        assert_eq!(read.transform.stretch, Mat3::IDENTITY);
        assert_eq!(
            read.hollow.built.map(|built| built.shape()),
            Some(Mat3::from_diagonal(Vec3::new(2.0, 1.0, 0.5))),
            "an older shell was measured under the size of its scale"
        );
    }

    #[test]
    fn an_empty_plate_round_trips() {
        let back = round_trip(&project(0));
        assert!(back.manifest.objects.is_empty());
        assert!(back.models.is_empty());
    }

    #[test]
    fn a_project_of_another_format_version_is_refused() {
        for found in [WHOLE_PROFILES - 1, VERSION + 1] {
            let mut saved = project(1);
            saved.manifest.version = found;

            let mut bytes = Cursor::new(Vec::new());
            write_to(&mut bytes, &saved).expect("a project in memory writes");
            bytes.set_position(0);

            let error = read_from(bytes).expect_err("the format is not this one");
            assert!(matches!(error, ProjectError::WrongVersion { found: it } if it == found));
        }
    }

    #[test]
    fn a_plate_brings_its_printer_by_id_and_its_picture_back() {
        let saved = project(1);
        let back = round_trip(&saved);
        assert_eq!(back.manifest.printer, saved.manifest.printer);
        assert_eq!(back.manifest.resin, None);
        assert_eq!(back.preview, saved.preview);

        let mut bare = project(1);
        bare.preview = None;
        assert_eq!(round_trip(&bare).preview, None, "a file without one reads");
    }

    #[test]
    fn a_version_4_file_keeps_the_ids_of_the_profiles_it_carried() {
        let saved = project(1);
        let mut older = serde_json::to_value(&saved.manifest).expect("serialises");
        older["version"] = WHOLE_PROFILES.into();
        older["printer"] = serde_json::json!({ "id": "my-saturn", "profile": { "name": "Mine" } });
        older["resin"] = serde_json::json!({ "id": null, "profile": { "name": "From a file" } });
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut bytes);
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file(MANIFEST, options)
                .expect("a zip in memory takes an entry");
            zip.write_all(&serde_json::to_vec(&older).expect("the manifest serialises"))
                .expect("a zip in memory takes bytes");
            for (name, mesh) in [
                (source_name(0), &saved.models[0].source),
                (shell_name(0), &saved.models[0].source),
            ] {
                zip.start_file(name, options)
                    .expect("a zip in memory takes an entry");
                zip.write_all(&blob::write(mesh))
                    .expect("a zip in memory takes bytes");
            }
            zip.finish().expect("a zip in memory finishes");
        }
        bytes.set_position(0);

        let back = read_from(bytes).expect("a version 4 file reads");
        assert_eq!(back.manifest.version, VERSION);
        assert_eq!(back.manifest.printer.as_deref(), Some("my-saturn"));
        assert_eq!(back.manifest.resin, None, "a resin with no id names none");
        assert_eq!(back.preview, None);
    }

    #[test]
    fn a_project_written_to_disk_reads_back() {
        let path = std::env::temp_dir().join("encrust-project-round-trip.encrust");
        let saved = project(1);
        save(&path, &saved).expect("the temporary directory is writable");

        let back = load(&path).expect("what was just written reads");
        assert_eq!(back.manifest.objects.len(), 1);
        assert_eq!(back.models[0].source, saved.models[0].source);
        std::fs::remove_file(&path).expect("the file was just written");
    }

    #[test]
    fn a_file_that_is_not_a_zip_is_refused() {
        let error = read_from(Cursor::new(b"# not a project".to_vec()))
            .expect_err("a text file is not a project");
        assert!(matches!(error, ProjectError::NotAProject(_)));
    }

    #[test]
    fn a_zip_without_a_manifest_is_refused() {
        let mut bytes = Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(&mut bytes);
        zip.start_file("readme.txt", zip::write::SimpleFileOptions::default())
            .expect("a zip in memory takes an entry");
        zip.finish().expect("a zip in memory finishes");
        bytes.set_position(0);

        let error = read_from(bytes).expect_err("there is no manifest in it");
        assert!(matches!(error, ProjectError::NotAProject(_)));
    }

    #[test]
    fn a_project_missing_a_mesh_is_refused() {
        let saved = project(2);
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut bytes);
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file(MANIFEST, options)
                .expect("a zip in memory takes an entry");
            zip.write_all(&serde_json::to_vec(&saved.manifest).expect("the manifest serialises"))
                .expect("a zip in memory takes bytes");
            zip.start_file(source_name(0), options)
                .expect("a zip in memory takes an entry");
            zip.write_all(&blob::write(&saved.models[0].source))
                .expect("a zip in memory takes bytes");
            zip.finish().expect("a zip in memory finishes");
        }
        bytes.set_position(0);

        let error = read_from(bytes).expect_err("the first model has no shell in the file");
        assert!(matches!(error, ProjectError::BadMesh { .. }));
    }
}
