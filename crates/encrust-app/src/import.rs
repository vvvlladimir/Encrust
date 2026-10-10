use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use core_geometry::{
    DEFAULT_WELD_TOLERANCE, Heightmap, Transform, Welded, center_over_plate, diagnose,
    drop_to_plate, orient_outward, weld,
};
use core_mesh_io::{Loaded, ModelFile, loader_for_extension};

use crate::camera::OrbitCamera;
use crate::files::{self, Handed, Wanted};
use crate::job::{ImportJob, ImportOutcome, ImportStage};
use crate::panels::frame_view;
use crate::plate::BuildPlate;
use crate::repair::Repairs;
use crate::scene::{ImportSummary, Imported, Mapped, Scene};
use crate::status::Status;

/// The imports currently being read, one thread each.
///
/// Opening a model is seconds of work on a real part, so none of it happens on the thread
/// that draws; see `docs/decisions/0038`. Several files dropped at once open at once.
#[derive(Debug, Default)]
pub struct Imports {
    jobs: Vec<ImportJob>,
    /// The files handed over since the window last took them, for the recent list.
    opened: Vec<PathBuf>,
}

impl Imports {
    /// Asks for a mesh file and starts opening it.
    pub fn open_dialog(&mut self, plate: &BuildPlate, status: &mut Status) {
        if let Some(file) = files::pick(Wanted::Model) {
            self.open(file, plate, status);
        }
    }

    /// Starts opening a mesh. The one way a model reaches the plate.
    pub fn open(&mut self, file: Handed, plate: &BuildPlate, status: &mut Status) {
        *status = Status::Info(format!("Opening {}", file.path().display()));
        if let Handed::Path(path) = &file {
            self.opened.push(path.clone());
        }
        self.jobs.push(ImportJob::spawn(file, plate.clone()));
    }

    /// The files handed over since the last call.
    pub fn take_opened(&mut self) -> Vec<PathBuf> {
        std::mem::take(&mut self.opened)
    }

    pub fn is_busy(&self) -> bool {
        !self.jobs.is_empty()
    }

    /// What the stage notice says while files are being read.
    pub fn label(&self) -> Option<String> {
        let first = self.jobs.first()?;
        Some(match self.jobs.len() {
            1 => first.label(),
            rest => format!("{}, and {} more", first.label(), rest - 1),
        })
    }

    /// Puts every import that has finished into the scene, points the camera at what
    /// arrived and asks about anything that came in broken. Returns whether one is still
    /// running, which is what tells the window to keep repainting.
    pub fn poll(
        &mut self,
        scene: &mut Scene,
        plate: &BuildPlate,
        camera: &mut OrbitCamera,
        repairs: &mut Repairs,
        status: &mut Status,
    ) -> bool {
        let mut opened = Vec::new();
        self.jobs.retain_mut(|job| match job.poll() {
            None => true,
            Some(outcome) => {
                opened.push((job.path().to_path_buf(), outcome));
                false
            }
        });

        let reported = !opened.is_empty();
        for (path, outcome) in opened {
            match outcome {
                ImportOutcome::Opened(imported) => {
                    let id = scene.insert(*imported);
                    if let Some(object) = scene.get(id) {
                        repairs.consider(object);
                    }
                    *status = Status::Info(format!("Opened {}", path.display()));
                    frame_view(scene, plate, camera);
                }
                ImportOutcome::Failed(message) => *status = Status::Error(message),
            }
        }

        // What just finished is the news; the ones still reading say so on a later frame.
        if !reported && let Some(label) = self.label() {
            *status = Status::Info(label);
        }
        self.is_busy()
    }
}

/// Loads a mesh, repairs it and stands it on the middle of the plate.
///
/// The repair order matches the CLI's: weld first, because on an unwelded mesh every edge
/// looks like a boundary and neither the orientation fix nor the diagnostics mean
/// anything. `stage` is called before each part of the work starts.
pub fn prepare(
    file: &Handed,
    plate: &BuildPlate,
    stage: &mut dyn FnMut(ImportStage),
) -> Result<Imported> {
    let path = file.path();
    let name = file_name(path);
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .with_context(|| format!("{name} has no file extension"))?;

    let loader = loader_for_extension(extension).with_context(|| format!("cannot load {name}"))?;

    stage(ImportStage::Reading);
    let loaded = match file {
        Handed::Path(path) => loader.load(path),
        Handed::Bytes { bytes, .. } => loader.read(ModelFile {
            path,
            source: &mut std::io::Cursor::new(&bytes[..]),
            beside: &|name| file.beside(name),
        }),
    }
    .with_context(|| format!("cannot load {name}"))?;

    stage(ImportStage::Repairing);
    // Repaired in place and moved out at the end, so a mesh of tens of megabytes is
    // never held twice.
    let mut welded = weld(&loaded.mesh, DEFAULT_WELD_TOLERANCE);
    // Kept only where there is a map to carry through the repair, which is the one thing
    // that renumbers and turns round the faces it is indexed by.
    let before = loaded.uvs.is_some().then(|| welded.mesh.faces.clone());
    let orientation = orient_outward(&mut welded.mesh);
    let mapped = before.and_then(|before| mapped(&loaded, &welded, &before));
    let diagnostics = diagnose(&welded.mesh);
    let summary = ImportSummary::new(&welded, orientation, diagnostics);
    let mesh = welded.mesh;

    let bounds = mesh
        .aabb()
        .with_context(|| format!("{name} has no vertices"))?;
    let placement = drop_to_plate(&bounds) + center_over_plate(&bounds, plate.x_mm, plate.y_mm);

    stage(ImportStage::Indexing);
    Ok(Imported {
        mapped,
        ..Imported::new(
            name,
            Arc::new(mesh),
            Transform::from_translation(placement),
            summary,
        )
    })
}

/// The file's own name, extension and all: three copies of one cube opened from an STL, an
/// OBJ and a 3MF are otherwise one name three times over. It is also what a failure is
/// reported under, since a reader's own error already carries the whole path.
fn file_name(path: &std::path::Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// The file's textures, over the mesh as the repair left it.
///
/// A file that carries coordinates but no image it can read is not mapped: there would be
/// nothing to press. One image it cannot read among several is refused whole rather than
/// silently renumbering the rest, which the map is indexed by.
fn mapped(loaded: &Loaded, welded: &Welded, before: &[[u32; 3]]) -> Option<Arc<Mapped>> {
    if loaded.textures.is_empty() {
        return None;
    }
    let heights: Vec<Heightmap> = loaded
        .textures
        .iter()
        .map(|texture| {
            texture
                .decode()
                .inspect_err(|error| tracing::warn!(%error, "the texture cannot be read"))
                .ok()
        })
        .collect::<Option<_>>()?;
    let flipped: Vec<u32> = before
        .iter()
        .zip(&welded.mesh.faces)
        .enumerate()
        .filter(|(_, (was, is))| was != is)
        .map(|(face, _)| face as u32)
        .collect();

    Some(Arc::new(Mapped {
        names: loaded
            .textures
            .iter()
            .map(|texture| texture.name.clone())
            .collect(),
        uvs: loaded
            .uvs
            .as_ref()?
            .without(&welded.dropped)
            .flipping(&flipped),
        heights,
    }))
}

/// Puts an already placed object back in the middle of the plate, standing on z = 0.
pub fn recenter(scene: &mut Scene, plate: &BuildPlate, index: usize) -> Option<()> {
    let object = scene.objects_mut().get_mut(index)?;
    let bounds = object.world_bounds()?;
    object.transform.translation +=
        drop_to_plate(&bounds) + center_over_plate(&bounds, plate.x_mm, plate.y_mm);
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    /// Prepares a file with the stages thrown away, which is what the tests care about.
    fn prepared(path: &Path) -> Result<Imported> {
        prepare(
            &Handed::Path(path.to_owned()),
            &BuildPlate::default(),
            &mut |_| {},
        )
    }

    #[test]
    fn a_file_without_an_extension_is_rejected() {
        let error = prepared(Path::new("model")).expect_err("a file with no extension");
        assert!(error.to_string().contains("extension"));
    }

    #[test]
    fn an_unsupported_extension_is_rejected() {
        assert!(prepared(&fixture("model.gcode")).is_err());
    }

    /// BUG-23: the stage notice has one line, and a reader's own error already carries the
    /// whole path, so what we add to it names the file alone.
    #[test]
    fn a_failure_names_the_file_and_not_the_path_it_was_opened_from() {
        let error = prepared(&fixture("model.gcode")).expect_err("no loader reads a program");
        let first = error.to_string();
        assert_eq!(first, "cannot load model.gcode");
        assert!(
            !first.contains(std::path::MAIN_SEPARATOR),
            "the path is the reader's to state, got {first}"
        );
    }

    #[test]
    fn a_prepared_model_carries_a_hierarchy_of_its_own_mesh() {
        let imported = prepared(&fixture("cube.stl")).expect("the fixture is a sound cube");
        assert!(!imported.mesh.is_empty());
        assert!(
            !imported.bvh.is_empty(),
            "a model is indexed before it lands"
        );
        assert!(
            imported.transform.translation.z.abs() < 1e-5,
            "an import stands on the plate, got {}",
            imported.transform.translation
        );
    }

    #[test]
    fn a_model_is_named_after_its_file_with_the_extension_on_it() {
        let imported = prepared(&fixture("cube.stl")).expect("a sound cube opens");
        assert_eq!(imported.name, "cube.stl");
    }

    #[test]
    fn a_model_handed_over_as_bytes_opens_as_it_does_from_disk() {
        let bytes = std::fs::read(fixture("cube.stl")).expect("the fixture is checked in");
        let handed = Handed::bytes("cube.stl", bytes.into());
        let imported =
            prepare(&handed, &BuildPlate::default(), &mut |_| {}).expect("a sound cube opens");
        let from_disk = prepared(&fixture("cube.stl")).expect("a sound cube opens");

        assert_eq!(imported.name, "cube.stl");
        assert_eq!(imported.mesh.faces.len(), from_disk.mesh.faces.len());
    }
}
