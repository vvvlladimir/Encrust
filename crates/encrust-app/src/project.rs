//! The `.encrust` project file: what the plate holds, written to be opened again.
//!
//! A zip of one JSON manifest and one mesh blob per model. Only what the user typed or
//! clicked is in it; everything the window can work out again — hierarchies, support
//! trees, cavities, the slice stack — is left out and rebuilt on load. See
//! `docs/formats/encrust-project.md` and `docs/decisions/0097`.

mod blob;
pub mod state;

use std::io::{Read, Seek, Write};
use std::num::NonZeroU8;
use std::path::Path;
use std::sync::Arc;

use anyhow::Context as _;
use core_format::ExposureRange;
use core_geometry::{Mesh, MeshDiagnostics, Orientation, Scalar, Transform};
use core_slicer::AdaptiveSettings;
use core_supports::{ProjectSettings, Region, SupportPoint, SupportTree};
use core_volume::{Blocker, Channel, DrainHole, HollowMode, InfillSettings};
use printer_profiles::{MaterialProfile, OutputFormat, PrinterProfile, SupportProfile};
use serde::{Deserialize, Serialize};

use crate::cut::Keep;
use crate::panels::Window;
use crate::panels::frame_view;
use crate::scene::Axis;
use crate::scene::Scene;
use crate::status::Status;
use crate::undo::History;
use crate::workspace::Array;

/// What a project file is called and what the open and save dialogs filter on.
pub const EXTENSION: &str = "encrust";

/// The manifest this build writes. A file claiming a higher one is refused rather than
/// read as far as it parses.
const VERSION: u32 = 1;

const MANIFEST: &str = "project.json";

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

/// The project file the window has open, so that Save knows where to write and the title
/// strip knows what to call the plate.
#[derive(Debug, Default)]
pub struct Opened {
    pub path: Option<std::path::PathBuf>,
    /// The digest of the plate as it was last written or read, or `None` for a plate that
    /// has never been either. Unsaved work is this not matching the plate in hand.
    saved: Option<u64>,
    /// The window is trying to close and is waiting to be told what to do about the work
    /// that is not saved.
    asking: bool,
    /// The answer was to close anyway, so the next close request is not questioned again.
    closing: bool,
}

impl Opened {
    /// What the title strip calls the plate: the file's own name, or nothing at all for a
    /// plate that has never been saved.
    pub fn name(&self) -> Option<&str> {
        self.path.as_ref()?.file_stem()?.to_str()
    }
}

/// What the plate reduces to, for telling a saved one from an edited one.
///
/// Read on demand rather than every frame: it captures the plate, which clones every
/// painted patch. Asked when the window is being closed, and nowhere else.
pub(crate) fn digest(manifest: &Manifest) -> u64 {
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

#[derive(Debug, Serialize, Deserialize)]
pub struct SupportState {
    /// One entry per group, group 0 first; see `docs/decisions/0094`.
    pub groups: Vec<Group>,
    pub active: u16,
    pub fill: ProjectSettings,
    pub brush_radius_mm: Scalar,
    pub flood_angle_deg: Scalar,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Group {
    pub name: String,
    pub profile: SupportProfile,
}

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

#[derive(Debug, Serialize, Deserialize)]
pub struct CutState {
    pub axis: Axis,
    pub height_mm: Scalar,
    pub keep: Keep,
}

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

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ObjectSupportState {
    pub points: Vec<SupportPoint>,
    pub painted: Region,
    pub blocked: Region,
    pub frozen: Vec<SupportTree>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ObjectHollowState {
    pub blockers: Vec<Blocker>,
    pub drains: Vec<DrainHole>,
    pub channels: Vec<Channel>,
}

/// Writes the project to `path`, replacing whatever was there.
pub fn save(path: &Path, project: &Project) -> Result<(), ProjectError> {
    let file = std::fs::File::create(path)?;
    write_to(std::io::BufWriter::new(file), project)
}

/// Reads a project back. A file from a newer build is refused whole: reading as far as
/// the manifest parses would silently drop whatever that build added.
pub fn load(path: &Path) -> Result<Project, ProjectError> {
    let file = std::fs::File::open(path)?;
    read_from(std::io::BufReader::new(file))
}

fn write_to<W: Write + Seek>(sink: W, project: &Project) -> Result<(), ProjectError> {
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

fn read_from<R: Read + Seek>(source: R) -> Result<Project, ProjectError> {
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

/// Takes everything off the plate and forgets which file it came from. The chosen
/// profiles and the tools' own numbers stay: they are how this user works, not what is
/// on the plate.
pub fn clear(window: &mut Window) {
    window.doc.scene = Scene::default();
    window.doc.history = History::default();
    window.doc.project.path = None;
    window.doc.project.saved = Some(digest(&captured(window).manifest));
    window.machine.status = Status::Info("New plate".to_owned());
}

/// Opens a project over whatever the window was holding.
pub fn open_dialog(window: &mut Window) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("Encrust project", &[EXTENSION])
        .pick_file()
    else {
        return;
    };
    let read = load(&path)
        .map_err(anyhow::Error::new)
        .with_context(|| format!("cannot open the project {}", path.display()));
    let Some(project) = window
        .machine
        .status
        .report(&format!("Opened {}", path.display()), read)
    else {
        return;
    };

    state::apply(
        project,
        state::PlateMut {
            scene: &mut window.doc.scene,
            slicing: &mut window.machine.slicing,
            supports: &mut window.tools.supports,
            hollow: &mut window.tools.hollow,
            drain: &mut window.tools.drain,
            cut: &mut window.tools.cut,
            array: &mut window.tools.array,
        },
    );
    window.doc.project.path = Some(path);
    window.doc.project.saved = Some(digest(&captured(window).manifest));
    window.doc.history = History::default();
    frame_view(
        &window.doc.scene,
        &window.doc.plate,
        &mut window.view.camera,
    );
}

/// Writes over the file the plate came from, or asks for a name when it came from none.
pub fn save_open(window: &mut Window) {
    match window.doc.project.path.clone() {
        Some(path) => write(window, path),
        None => save_dialog(window),
    }
}

pub fn save_dialog(window: &mut Window) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("Encrust project", &[EXTENSION])
        .set_file_name(format!("plate.{EXTENSION}"))
        .save_file()
    else {
        return;
    };
    write(window, with_extension(path));
}

fn write(window: &mut Window, path: std::path::PathBuf) {
    let project = captured(window);
    let digest = digest(&project.manifest);
    let written = save(&path, &project)
        .map_err(anyhow::Error::new)
        .with_context(|| format!("cannot write the project {}", path.display()));
    if window
        .machine
        .status
        .report(&format!("Saved {}", path.display()), written)
        .is_some()
    {
        window.doc.project.path = Some(path);
        window.doc.project.saved = Some(digest);
    }
}

/// The plate as a project, which is both what gets written and what gets hashed.
fn captured(window: &Window) -> Project {
    state::capture(state::Plate {
        scene: &window.doc.scene,
        slicing: &window.machine.slicing,
        supports: &window.tools.supports,
        hollow: &window.tools.hollow,
        drain: &window.tools.drain,
        cut: &window.tools.cut,
        array: &window.tools.array,
    })
}

/// The empty plate the window opens on counts as saved: closing it again asks nothing.
pub fn mark_saved(window: &mut Window) {
    window.doc.project.saved = Some(digest(&captured(window).manifest));
}

/// Whether the plate has moved since it was last written or read.
fn is_dirty(window: &Window) -> bool {
    window.doc.project.saved != Some(digest(&captured(window).manifest))
}

/// Holds the window open when there is work in it nobody has written down, and asks what
/// to do about it. Nothing is asked of a plate that matches its file.
pub fn guard_close(ui: &egui::Ui, window: &mut Window) {
    if window.doc.project.closing {
        return;
    }
    let asked = ui.ctx().input(|input| input.viewport().close_requested());
    if asked && !window.doc.project.asking {
        if !is_dirty(window) {
            return;
        }
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::CancelClose);
        window.doc.project.asking = true;
    }
    if !window.doc.project.asking {
        return;
    }

    let mut close = false;
    egui::Modal::new(egui::Id::new("unsaved")).show(ui.ctx(), |ui| {
        ui.label("This plate has changes that are not in a project file.");
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Save and close").clicked() {
                save_open(window);
                // A dialog the user backed out of leaves the plate unsaved, and the
                // window open with it.
                close = !is_dirty(window);
                window.doc.project.asking = false;
            }
            if ui.button("Close without saving").clicked() {
                close = true;
                window.doc.project.asking = false;
            }
            if ui.button("Keep working").clicked() {
                window.doc.project.asking = false;
                window.machine.updates.cancel_restart();
            }
        });
    });
    if close {
        window.doc.project.closing = true;
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

/// A name typed without the extension is still a project file; a dialog that already
/// applied it is left alone.
fn with_extension(path: std::path::PathBuf) -> std::path::PathBuf {
    if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case(EXTENSION))
    {
        path
    } else {
        path.with_extension(EXTENSION)
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
        assert_eq!(back.manifest.slicing.format, OutputFormat::Ctb5);
        assert_eq!(back.manifest.hollow.mode, HollowMode::BottomThrough);
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
