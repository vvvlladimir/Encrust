//! The `.encrust` project file: what the plate holds, written to be opened again.
//!
//! A zip of one JSON manifest and one mesh blob per model. Only what the user typed or
//! clicked is in it; everything a front end can work out again — hierarchies, support
//! trees, cavities, the slice stack — is left out and rebuilt on load. Data and
//! (de)serialisation only: nothing here knows about a window. See
//! `docs/formats/encrust-project.md` and `docs/decisions/0097`.

mod blob;

use std::io::{Read, Seek, Write};
use std::num::NonZeroU8;
use std::path::Path;
use std::sync::Arc;

use core_format::ExposureRange;
use core_geometry::{Mesh, MeshDiagnostics, Orientation, Scalar, Transform};
use core_slicer::AdaptiveSettings;
use core_supports::{ProjectSettings, Region, SupportPoint, SupportTree};
use core_volume::{Blocker, Channel, DrainHole, HollowMode, HollowSettings, InfillSettings};
use printer_profiles::{MaterialProfile, OutputFormat, PrinterProfile, SupportProfile};
use serde::{Deserialize, Serialize};

/// What a project file is called, and what an open or save dialog filters on.
pub const EXTENSION: &str = "encrust";

/// The manifest this build writes. A file claiming a higher one is refused rather than
/// read as far as it parses.
pub const VERSION: u32 = 2;

const MANIFEST: &str = "project.json";

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
        "this project was written by a newer Encrust (format {found}, this build reads {VERSION})"
    )]
    TooNew { found: u32 },
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

/// A project as the window holds it: the manifest, and one mesh per object in the same
/// order as [`Manifest::objects`].
#[derive(Debug)]
pub struct Project {
    pub manifest: Manifest,
    pub meshes: Vec<Arc<Mesh>>,
}

/// Everything a project holds but its meshes, as `project.json` stores it.
#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub version: u32,
    pub printer: Option<Chosen<PrinterProfile>>,
    pub resin: Option<Chosen<MaterialProfile>>,
    pub slicing: SlicingState,
    pub supports: SupportState,
    pub hollow: HollowState,
    pub drain: DrainState,
    pub cut: CutState,
    pub array: Array,
    /// One name per plate. A file written before there were plates has none, and opens
    /// as the single plate every object stood on.
    #[serde(default)]
    pub plates: Vec<String>,
    #[serde(default)]
    pub active_plate: u32,
    pub objects: Vec<ObjectState>,
}

/// A profile as it was in hand, with the catalogue id it came from when it had one. The
/// profile travels with the file so that a plate opens the same on a machine whose
/// catalogue does not have that id.
#[derive(Debug, Serialize, Deserialize)]
pub struct Chosen<T> {
    pub id: Option<String>,
    pub profile: T,
}

fn one_sample() -> NonZeroU8 {
    core_slicer::ONE_SAMPLE
}

/// How the plate is cut and drawn: layer height, exposures, anti-aliasing, the container.
#[derive(Debug, Serialize, Deserialize)]
pub struct SlicingState {
    pub layer_height_mm: Scalar,
    pub adaptive: Option<AdaptiveSettings>,
    pub exposure: Vec<ExposureRange>,
    /// Absent in a project written before step 16d, which meant one plane a layer.
    #[serde(default = "one_sample")]
    pub samples: NonZeroU8,
    pub anti_alias: bool,
    /// Absent in a project written before step 16c, which meant all 255 greys.
    #[serde(default)]
    pub grey_levels: Option<NonZeroU8>,
    /// Absent in a project written before step 16f, which meant sharp edges.
    #[serde(default)]
    pub blur_px: u8,
    /// Absent in a project written before step 17b, which kept its islands.
    #[serde(default)]
    pub remove_islands: bool,
    pub format: OutputFormat,
}

/// The support tool as it was left: its groups and what the next support is drawn with.
#[derive(Debug, Serialize, Deserialize)]
pub struct SupportState {
    /// One entry per group, group 0 first; see `docs/decisions/0094`.
    pub groups: Vec<Group>,
    pub active: u16,
    pub fill: ProjectSettings,
    pub brush_radius_mm: Scalar,
    pub flood_angle_deg: Scalar,
}

/// A named set of support settings that supports are drawn with.
#[derive(Debug, Serialize, Deserialize)]
pub struct Group {
    pub name: String,
    pub profile: SupportProfile,
}

/// What the next hollowing is done with. A hollowed model carries its own [`Cavity`].
#[derive(Debug, Serialize, Deserialize)]
pub struct HollowState {
    pub thickness_mm: Scalar,
    pub mode: HollowMode,
    pub precision: Scalar,
    pub infill_on: bool,
    pub infill: InfillSettings,
    pub blocker_mm: Scalar,
}

/// What the next drain hole is drilled to. The holes standing carry their own sizes.
#[derive(Debug, Serialize, Deserialize)]
pub struct DrainState {
    pub diameter_mm: Scalar,
    pub depth_mm: Scalar,
    pub taper: Scalar,
}

/// Where the cut tool's plane stands and which halves it keeps.
#[derive(Debug, Serialize, Deserialize)]
pub struct CutState {
    pub axis: Axis,
    /// Across Z, millimetres above the plate; across X or Y, millimetres from the model's
    /// centre of mass. Named for the Z cut it began as.
    pub height_mm: Scalar,
    pub keep: Keep,
}

/// One model on a plate: where it stands, and what was done to it.
#[derive(Debug, Serialize, Deserialize)]
pub struct ObjectState {
    pub name: String,
    #[serde(default)]
    pub plate: u32,
    pub transform: Transform,
    pub visible: bool,
    pub summary: Summary,
    pub supports: ObjectSupportState,
    pub hollow: ObjectHollowState,
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

/// One model's supports: the points placed, the regions painted and the trees kept.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ObjectSupportState {
    pub points: Vec<SupportPoint>,
    pub painted: Region,
    pub blocked: Region,
    pub frozen: Vec<SupportTree>,
}

/// One model's hollowing: its cavity, and the blockers, drains and channels cut into it.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ObjectHollowState {
    pub blockers: Vec<Blocker>,
    pub drains: Vec<DrainHole>,
    pub channels: Vec<Channel>,
    /// What the model was hollowed with, or `None` for a solid one. Absent in a version 1
    /// file, which kept no cavity at all.
    #[serde(default)]
    pub cavity: Option<Cavity>,
}

/// The wall a model was hollowed to, which is all a cavity needs to be built again: its
/// blockers and channels are already beside it (ADR 0178).
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
    for (index, mesh) in project.meshes.iter().enumerate() {
        zip.start_file(mesh_name(index), options)?;
        zip.write_all(&blob::write(mesh))?;
    }
    zip.finish()?;
    Ok(())
}

/// Writes the project to `path`, replacing whatever was there.
pub fn save(path: &Path, project: &Project) -> Result<(), ProjectError> {
    let file = std::fs::File::create(path)?;
    write_to(std::io::BufWriter::new(file), project)
}

/// Reads a project back from `source`. A file from a newer build is refused whole:
/// reading as far as the manifest parses would silently drop whatever that build added.
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
        if version.version > VERSION {
            return Err(ProjectError::TooNew {
                found: version.version,
            });
        }
        serde_json::from_str(&text)?
    };

    let mut meshes = Vec::with_capacity(manifest.objects.len());
    for index in 0..manifest.objects.len() {
        let name = mesh_name(index);
        let mut entry = zip
            .by_name(&name)
            .map_err(|_| ProjectError::BadMesh { name: name.clone() })?;
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        meshes.push(Arc::new(blob::read(&name, &bytes)?));
    }

    Ok(Project { manifest, meshes })
}

/// Reads a project back from a file at `path`.
pub fn load(path: &Path) -> Result<Project, ProjectError> {
    let file = std::fs::File::open(path)?;
    read_from(std::io::BufReader::new(file))
}

/// The version alone, read before the rest so that a manifest from a newer build is
/// refused by its number rather than by a field it happens to have renamed.
#[derive(Deserialize)]
struct Version {
    version: u32,
}

fn mesh_name(index: usize) -> String {
    format!("models/{index}.mesh")
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

    fn project(objects: usize) -> Project {
        let mesh = Mesh::new(vec![Vec3::ZERO, Vec3::X, Vec3::Y], vec![[0, 1, 2]]);
        Project {
            manifest: Manifest {
                version: VERSION,
                printer: None,
                resin: None,
                slicing: SlicingState {
                    layer_height_mm: 0.05,
                    adaptive: Some(AdaptiveSettings::default()),
                    exposure: vec![ExposureRange::new(0.0, 4.0, 9.5)],
                    samples: one_sample(),
                    anti_alias: true,
                    grey_levels: None,
                    blur_px: 0,
                    remove_islands: false,
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
                },
                hollow: HollowState {
                    thickness_mm: 2.0,
                    mode: HollowMode::BottomThrough,
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
                },
                array: Array::default(),
                plates: vec!["Plate 1".to_owned()],
                active_plate: 0,
                objects: (0..objects)
                    .map(|index| ObjectState {
                        name: format!("model {index}"),
                        plate: 0,
                        transform: Transform::from_translation(Vec3::new(1.0, 2.0, 3.0)),
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
                                shells: 1,
                                euler_characteristic: 1,
                            },
                        },
                        supports: ObjectSupportState {
                            points: vec![SupportPoint::new(Vec3::Z).in_group(1)],
                            painted: marked(&[0]),
                            blocked: Region::default(),
                            frozen: Vec::new(),
                        },
                        hollow: ObjectHollowState {
                            blockers: vec![Blocker::ball(Vec3::ZERO, 1.5)],
                            drains: Vec::new(),
                            channels: vec![Channel {
                                points: vec![Vec3::ZERO, Vec3::Z],
                                diameter_mm: 3.0,
                            }],
                            cavity: Some(Cavity {
                                thickness_mm: 1.5,
                                mode: HollowMode::BottomThrough,
                                precision: 0.25,
                                infill: None,
                            }),
                        },
                    })
                    .collect(),
            },
            meshes: (0..objects).map(|_| Arc::new(mesh.clone())).collect(),
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
        assert_eq!(back.meshes, saved.meshes);
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
        assert_eq!(
            object.hollow.cavity,
            saved.manifest.objects[0].hollow.cavity
        );
        assert_eq!(back.manifest.slicing.format, OutputFormat::Ctb5);
        assert_eq!(back.manifest.hollow.mode, HollowMode::BottomThrough);
    }

    #[test]
    fn a_version_1_file_opens_with_every_model_solid() {
        let saved = project(1);
        let mut manifest = serde_json::to_value(&saved.manifest).expect("the manifest serialises");
        manifest["version"] = 1.into();
        let hollow = manifest["objects"][0]["hollow"]
            .as_object_mut()
            .expect("an object carries its hollow state");
        hollow.remove("cavity");

        let mut bytes = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut bytes);
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file(MANIFEST, options)
                .expect("a zip in memory takes an entry");
            zip.write_all(&serde_json::to_vec(&manifest).expect("the manifest serialises"))
                .expect("a zip in memory takes bytes");
            zip.start_file(mesh_name(0), options)
                .expect("a zip in memory takes an entry");
            zip.write_all(&blob::write(&saved.meshes[0]))
                .expect("a zip in memory takes bytes");
            zip.finish().expect("a zip in memory finishes");
        }
        bytes.set_position(0);

        let back = read_from(bytes).expect("a version 1 file still opens");
        assert_eq!(back.manifest.objects[0].hollow.cavity, None);
    }

    #[test]
    fn an_empty_plate_round_trips() {
        let back = round_trip(&project(0));
        assert!(back.manifest.objects.is_empty());
        assert!(back.meshes.is_empty());
    }

    #[test]
    fn a_project_from_a_newer_build_is_refused() {
        let mut saved = project(1);
        saved.manifest.version = VERSION + 1;

        let mut bytes = Cursor::new(Vec::new());
        write_to(&mut bytes, &saved).expect("a project in memory writes");
        bytes.set_position(0);

        let error = read_from(bytes).expect_err("the format is newer than this build");
        assert!(matches!(error, ProjectError::TooNew { found } if found == VERSION + 1));
    }

    #[test]
    fn a_project_written_to_disk_reads_back() {
        let path = std::env::temp_dir().join("encrust-project-round-trip.encrust");
        let saved = project(1);
        save(&path, &saved).expect("the temporary directory is writable");

        let back = load(&path).expect("what was just written reads");
        assert_eq!(back.manifest.objects.len(), 1);
        assert_eq!(back.meshes, saved.meshes);
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
            zip.start_file(mesh_name(0), options)
                .expect("a zip in memory takes an entry");
            zip.write_all(&blob::write(&saved.meshes[0]))
                .expect("a zip in memory takes bytes");
            zip.finish().expect("a zip in memory finishes");
        }
        bytes.set_position(0);

        let error = read_from(bytes).expect_err("the second model is not in the file");
        assert!(matches!(error, ProjectError::BadMesh { .. }));
    }
}
