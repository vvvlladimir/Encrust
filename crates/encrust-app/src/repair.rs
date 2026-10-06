//! What the window does about a model that arrived broken: the question it asks the
//! moment the model lands, and the runs that close the holes.
//!
//! Welding and orientation are done without asking, because both follow from the geometry
//! (ADR 0005). Closing a hole invents surface that was never in the file, so it is asked
//! for; see `docs/decisions/0194-a-hole-is-closed-only-when-it-is-asked-for.md` and 0195.

use std::sync::Arc;

use crate::job::{RepairJob, RepairOutcome, RepairRequest, Repaired};
use crate::scene::{ObjectId, Scene, SceneObject};
use crate::status::Status;
use crate::ui::hint;

/// How wide the question about a broken model is drawn, so its lines wrap rather than
/// stretching the window.
const ASK_W: f32 = 360.0;

/// A model the window has not been told what to do about yet.
#[derive(Debug, Clone, PartialEq)]
struct Asked {
    id: ObjectId,
    name: String,
    defects: Vec<String>,
}

/// The questions waiting to be answered, and the repairs under way.
#[derive(Debug, Default)]
pub struct Repairs {
    asked: Vec<Asked>,
    jobs: Vec<RepairJob>,
}

impl Repairs {
    /// Asks about a model that will not slice as it stands. A sound one is left alone, and
    /// so is one already being asked about.
    pub fn consider(&mut self, object: &SceneObject) {
        if object.summary.is_sound() || self.asked.iter().any(|asked| asked.id == object.id) {
            return;
        }
        self.asked.push(Asked {
            id: object.id,
            name: object.name.clone(),
            defects: object.summary.defects(),
        });
    }

    /// Starts closing the holes in one model, dropping any question about it.
    ///
    /// The mesh is replaced by the repaired one, so supports, a cavity and a texture
    /// placed on the model before this go with the mesh they were made for.
    pub fn start(&mut self, scene: &Scene, id: ObjectId, status: &mut Status) {
        self.asked.retain(|asked| asked.id != id);
        let Some(object) = scene.get(id) else {
            return;
        };
        let job = RepairJob::spawn(RepairRequest {
            id,
            name: object.name.clone(),
            mesh: Arc::clone(&object.mesh),
        });
        *status = Status::Info(job.label());
        self.jobs.push(job);
    }

    /// Leaves a model as it came, and stops asking about it.
    pub fn keep_as_it_is(&mut self, id: ObjectId) {
        self.asked.retain(|asked| asked.id != id);
    }

    /// The model the window is asking about: its identifier, what it is called and what is
    /// wrong with it.
    pub fn asking(&self) -> Option<(ObjectId, &str, &[String])> {
        let asked = self.asked.first()?;
        Some((asked.id, &asked.name, &asked.defects))
    }

    pub fn is_busy(&self) -> bool {
        !self.jobs.is_empty()
    }

    /// Puts every repair that has finished into the scene. Returns whether one is still
    /// running, which is what tells the window to keep repainting.
    pub fn poll(&mut self, scene: &mut Scene, status: &mut Status) -> bool {
        let mut done = Vec::new();
        self.jobs.retain_mut(|job| match job.poll() {
            None => true,
            Some(outcome) => {
                done.push(outcome);
                false
            }
        });

        for outcome in done {
            match outcome {
                RepairOutcome::Done(repaired) => {
                    *status = apply(scene, &repaired);
                }
                RepairOutcome::Failed(message) => *status = Status::Error(message),
            }
        }
        self.is_busy()
    }
}

/// Puts a repaired mesh back on the model it came from, and says what repairing it did.
fn apply(scene: &mut Scene, repaired: &Repaired) -> Status {
    let Some(object) = scene.get_mut(repaired.id) else {
        return Status::Error(format!("{} is no longer on the plate", repaired.name));
    };
    object.reshape(Arc::clone(&repaired.mesh));
    // What repair did is said in the status bar; the summary keeps describing the file,
    // so a mended model carries no mark of its own.
    object.summary.orientation = repaired.orientation;

    let name = &repaired.name;
    let left = repaired.filled.loops_left;
    match (done(repaired), left) {
        (0, 0) => Status::Info(format!("{name} had nothing left to repair")),
        (0, left) => Status::Error(format!(
            "{name} has {left} boundaries with no patch that would close them"
        )),
        (_, 0) => Status::Info(format!("Repaired {name}: {}", what(repaired).join(", "))),
        (_, left) => Status::Info(format!(
            "Repaired {name}: {}; {left} boundaries could not be closed",
            what(repaired).join(", ")
        )),
    }
}

/// How many things the repair actually changed, which is what decides whether it has
/// anything to report.
fn done(repaired: &Repaired) -> usize {
    repaired.filled.loops_filled + repaired.duplicates_removed
}

/// What it changed, in the words the status bar says it in.
fn what(repaired: &Repaired) -> Vec<String> {
    let mut lines = Vec::new();
    if repaired.filled.loops_filled > 0 {
        lines.push(format!("closed {} hole(s)", repaired.filled.loops_filled));
    }
    if repaired.duplicates_removed > 0 {
        lines.push(format!(
            "dropped {} face(s) drawn twice",
            repaired.duplicates_removed
        ));
    }
    lines
}

/// The question about a model that arrived broken, over everything else: its answer
/// decides what the mesh on the plate is, so nothing else is worth doing first.
pub fn ask(ui: &mut egui::Ui, repairs: &mut Repairs, scene: &Scene, status: &mut Status) {
    let Some((id, name, defects)) = repairs
        .asking()
        .map(|(id, name, defects)| (id, name.to_owned(), defects.to_vec()))
    else {
        return;
    };

    let mut answer = None;
    egui::Modal::new(egui::Id::new("broken-import")).show(ui.ctx(), |ui| {
        ui.set_max_width(ASK_W);
        ui.label(format!("{name} is broken."));
        for line in &defects {
            hint(ui, line);
        }
        ui.add_space(8.0);
        hint(
            ui,
            "Repairing covers what is open with surface that was not in the file. Left as \
             it is, the model stays as it came and what is sliced from it may not print.",
        );
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Repair").clicked() {
                answer = Some(true);
            }
            if ui.button("Keep as it is").clicked() {
                answer = Some(false);
            }
        });
    });

    match answer {
        Some(true) => repairs.start(scene, id, status),
        Some(false) => repairs.keep_as_it_is(id),
        None => {}
    }
}

/// How many models on the plate being edited will not slice as they stand, which is what
/// the footer reminds the user of before slicing.
pub fn broken_on_the_plate(scene: &Scene) -> usize {
    scene
        .printable(scene.active_plate())
        .filter(|object| !object.summary.is_sound())
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{ImportSummary, Imported};
    use core_geometry::{Mesh, MeshDiagnostics, Orientation, Transform, Vec3, diagnose};

    fn object(scene: &mut Scene, boundary_edges: usize) -> ObjectId {
        let mesh = Mesh::new(vec![Vec3::ZERO, Vec3::X, Vec3::Y], vec![[0, 1, 2]]);
        let mesh = Arc::new(mesh);
        scene.insert(Imported::new(
            "model.stl".to_owned(),
            Arc::clone(&mesh),
            Transform::default(),
            ImportSummary {
                vertices_merged: 0,
                faces_removed: 0,
                orientation: Orientation {
                    flipped_faces: 0,
                    inverted_shells: 0,
                    orientable: true,
                },
                diagnostics: MeshDiagnostics {
                    boundary_edges,
                    ..diagnose(&mesh)
                },
            },
        ))
    }

    #[test]
    fn a_sound_model_is_never_asked_about() {
        let mut scene = Scene::default();
        let id = object(&mut scene, 0);
        let mut repairs = Repairs::default();
        repairs.consider(scene.get(id).expect("the model was just inserted"));

        assert!(repairs.asking().is_none());
    }

    #[test]
    fn a_broken_model_is_asked_about_once() {
        let mut scene = Scene::default();
        let id = object(&mut scene, 3);
        let mut repairs = Repairs::default();
        let object = scene.get(id).expect("the model was just inserted");
        repairs.consider(object);
        repairs.consider(object);

        let (asked, name, defects) = repairs.asking().expect("an open model is a question");
        assert_eq!(asked, id);
        assert_eq!(name, "model.stl");
        assert_eq!(defects.len(), 1, "the open surface");
    }

    #[test]
    fn keeping_a_model_as_it_is_ends_the_question() {
        let mut scene = Scene::default();
        let id = object(&mut scene, 3);
        let mut repairs = Repairs::default();
        repairs.consider(scene.get(id).expect("the model was just inserted"));
        repairs.keep_as_it_is(id);

        assert!(repairs.asking().is_none());
        assert!(!repairs.is_busy(), "nothing was started");
    }

    #[test]
    fn a_broken_model_on_the_plate_is_counted_before_slicing() {
        let mut scene = Scene::default();
        object(&mut scene, 3);
        object(&mut scene, 0);

        assert_eq!(broken_on_the_plate(&scene), 1);
    }

    /// A box with the two triangles of its top missing, diagnosed for real, which is a
    /// model the window calls broken.
    fn open_box(scene: &mut Scene) -> ObjectId {
        let mesh = Arc::new(Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(1.0, 1.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
                Vec3::new(1.0, 0.0, 1.0),
                Vec3::new(1.0, 1.0, 1.0),
                Vec3::new(0.0, 1.0, 1.0),
            ],
            vec![
                [0, 2, 1],
                [0, 3, 2],
                [0, 1, 5],
                [0, 5, 4],
                [1, 2, 6],
                [1, 6, 5],
                [2, 3, 7],
                [2, 7, 6],
                [3, 0, 4],
                [3, 4, 7],
            ],
        ));
        scene.insert(Imported::new(
            "open.stl".to_owned(),
            Arc::clone(&mesh),
            Transform::default(),
            ImportSummary {
                vertices_merged: 0,
                faces_removed: 0,
                orientation: Orientation {
                    flipped_faces: 0,
                    inverted_shells: 0,
                    orientable: true,
                },
                diagnostics: diagnose(&mesh),
            },
        ))
    }

    #[test]
    fn a_model_that_came_back_closed_is_no_longer_called_broken() {
        let mut scene = Scene::default();
        let id = open_box(&mut scene);
        assert_eq!(broken_on_the_plate(&scene), 1);

        let mut mesh = (*scene.get(id).expect("the box is there").mesh).clone();
        let filled = core_geometry::fill_holes(&mut mesh);
        let orientation = core_geometry::orient_outward(&mut mesh);
        let status = apply(
            &mut scene,
            &Repaired {
                id,
                name: "open.stl".to_owned(),
                mesh: Arc::new(mesh),
                filled,
                duplicates_removed: 0,
                orientation,
            },
        );

        assert!(!status.is_error());
        let object = scene.get(id).expect("the box is still there");
        assert!(object.summary.is_sound());
        assert!(object.summary.defects().is_empty(), "the mark goes with it");
        assert_eq!(broken_on_the_plate(&scene), 0);
    }

    #[test]
    fn a_repair_of_a_model_that_has_been_deleted_is_reported_rather_than_applied() {
        let mut scene = Scene::default();
        let id = object(&mut scene, 3);
        let mesh = Arc::clone(&scene.get(id).expect("inserted").mesh);
        scene.remove(id);

        let status = apply(
            &mut scene,
            &Repaired {
                id,
                name: "model.stl".to_owned(),
                mesh,
                filled: core_geometry::Filled::default(),
                duplicates_removed: 0,
                orientation: Orientation {
                    flipped_faces: 0,
                    inverted_shells: 0,
                    orientable: true,
                },
            },
        );
        assert!(status.is_error());
    }
}
