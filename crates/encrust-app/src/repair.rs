//! What the window does about a model that arrived broken: the offer at the foot of the
//! stage, and the runs that close the holes.
//!
//! Welding, orientation and dropping the faces laid over others are done on import without
//! asking, because none of them changes what the model encloses (ADR 0005, 0270). Closing a
//! hole invents surface that was never in the file, so it is offered without blocking
//! anything and waits for the Repair button; see
//! `docs/decisions/0270-repair-cancels-what-encloses-nothing-and-never-worsens.md`.

use std::sync::Arc;

use crate::i18n::tr;
use crate::job::{RepairJob, RepairOutcome, RepairRequest, Repaired};
use crate::scene::{ObjectId, Scene, SceneObject};
use crate::status::Status;

/// The repairs under way, and the broken models the user chose to keep as they came.
#[derive(Debug, Default)]
pub struct Repairs {
    jobs: Vec<RepairJob>,
    kept: Vec<ObjectId>,
}

impl Repairs {
    /// The broken models on the plate being edited that repair is still offered for:
    /// neither kept as they came nor being repaired already.
    pub fn offered<'a>(&'a self, scene: &'a Scene) -> impl Iterator<Item = &'a SceneObject> {
        scene.printable(scene.active_plate()).filter(|object| {
            !object.summary.is_sound()
                && !self.kept.contains(&object.id)
                && !self.jobs.iter().any(|job| job.id() == object.id)
        })
    }

    /// Stops offering to repair a model. Its mark stays, and so does its Repair button.
    pub fn keep_as_it_is(&mut self, id: ObjectId) {
        if !self.kept.contains(&id) {
            self.kept.push(id);
        }
    }

    /// Starts closing the holes in one model.
    ///
    /// The mesh is replaced by the repaired one, so supports, a cavity and a texture
    /// placed on the model before this go with the mesh they were made for.
    pub fn start(&mut self, scene: &Scene, id: ObjectId, status: &mut Status) {
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
                done.push((job.id(), outcome));
                false
            }
        });

        for (id, outcome) in done {
            match outcome {
                RepairOutcome::Done(repaired) => {
                    *status = apply(scene, &repaired);
                }
                RepairOutcome::Failed(message) => *status = Status::Error(message),
            }
            // A repair that could not mend it is not offered again; the button still is.
            if scene
                .get(id)
                .is_some_and(|object| !object.summary.is_sound())
            {
                self.keep_as_it_is(id);
            }
        }
        self.is_busy()
    }
}

/// Puts a repaired mesh back on the model it came from, and says what repairing it did.
fn apply(scene: &mut Scene, repaired: &Repaired) -> Status {
    let Some(object) = scene.get_mut(repaired.id) else {
        return Status::Error(tr!("repair-gone", name = repaired.name.clone()));
    };
    if repaired.kept_as_found {
        return Status::Error(tr!("repair-kept", name = repaired.name.clone()));
    }
    object.reshape(repaired.shaped.clone());
    // What repair did is said in the stage notice; the summary keeps describing the file,
    // so a mended model carries no mark of its own.
    object.summary.orientation = repaired.orientation;

    let name = &repaired.name;
    let left = repaired.filled.loops_left;
    match (done(repaired), left) {
        (0, 0) => Status::Info(tr!("repair-nothing", name = name.clone())),
        (0, left) => Status::Error(tr!("repair-no-patch", name = name.clone(), count = left)),
        (_, 0) => Status::Info(tr!(
            "repair-done",
            name = name.clone(),
            what = what(repaired).join(", ")
        )),
        (_, left) => Status::Info(tr!(
            "repair-done-open",
            name = name.clone(),
            what = what(repaired).join(", "),
            count = left
        )),
    }
}

/// How many things the repair actually changed, which is what decides whether it has
/// anything to report.
fn done(repaired: &Repaired) -> usize {
    repaired.filled.loops_filled + repaired.duplicates_removed
}

/// What it changed, in the words the stage notice says it in.
fn what(repaired: &Repaired) -> Vec<String> {
    let mut lines = Vec::new();
    if repaired.filled.loops_filled > 0 {
        lines.push(tr!(
            "repair-closed-holes",
            count = repaired.filled.loops_filled
        ));
    }
    if repaired.duplicates_removed > 0 {
        lines.push(tr!(
            "repair-dropped-faces",
            count = repaired.duplicates_removed
        ));
    }
    lines
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
                    unbalanced_edges: boundary_edges,
                    ..diagnose(&mesh)
                },
            },
        ))
    }

    #[test]
    fn repair_is_offered_for_a_broken_model_and_not_a_sound_one() {
        let mut scene = Scene::default();
        let broken = object(&mut scene, 3);
        object(&mut scene, 0);
        let repairs = Repairs::default();

        let offered: Vec<ObjectId> = repairs.offered(&scene).map(|object| object.id).collect();
        assert_eq!(offered, vec![broken]);
    }

    #[test]
    fn a_model_kept_as_it_is_is_no_longer_offered() {
        let mut scene = Scene::default();
        let id = object(&mut scene, 3);
        let mut repairs = Repairs::default();
        repairs.keep_as_it_is(id);

        assert_eq!(repairs.offered(&scene).count(), 0);
        assert_eq!(broken_on_the_plate(&scene), 1, "it is still broken");
    }

    #[test]
    fn a_repair_that_could_not_mend_a_model_is_not_offered_again() {
        let mut scene = Scene::default();
        // A lone triangle would be closed by its own reverse and cancel away, so it is kept.
        let id = object(&mut scene, 3);
        let mut repairs = Repairs::default();
        repairs.start(&scene, id, &mut Status::default());
        while repairs.poll(&mut scene, &mut Status::default()) {
            std::thread::yield_now();
        }

        assert!(!scene.get(id).expect("still there").summary.is_sound());
        assert_eq!(repairs.offered(&scene).count(), 0);
    }

    #[test]
    fn a_model_under_repair_is_no_longer_offered() {
        let mut scene = Scene::default();
        let id = open_box(&mut scene);
        let mut repairs = Repairs::default();
        repairs.start(&scene, id, &mut Status::default());

        assert_eq!(repairs.offered(&scene).count(), 0);
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
                shaped: crate::scene::Shaped::of(Arc::new(mesh)),
                filled,
                duplicates_removed: 0,
                orientation,
                kept_as_found: false,
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
                shaped: crate::scene::Shaped::of(mesh),
                filled: core_geometry::Filled::default(),
                duplicates_removed: 0,
                orientation: Orientation {
                    flipped_faces: 0,
                    inverted_shells: 0,
                    orientable: true,
                },
                kept_as_found: false,
            },
        );
        assert!(status.is_error());
    }

    #[test]
    fn a_repair_that_would_worsen_the_model_leaves_it_as_it_came() {
        let mut scene = Scene::default();
        let id = open_box(&mut scene);
        let before = Arc::clone(&scene.get(id).expect("the box is there").mesh);

        let status = apply(
            &mut scene,
            &Repaired {
                id,
                name: "open.stl".to_owned(),
                shaped: crate::scene::Shaped::of(Arc::new(Mesh::default())),
                filled: core_geometry::Filled::default(),
                duplicates_removed: 0,
                orientation: Orientation {
                    flipped_faces: 0,
                    inverted_shells: 0,
                    orientable: true,
                },
                kept_as_found: true,
            },
        );

        assert!(status.is_error(), "the user is told nothing was changed");
        let object = scene.get(id).expect("the box is still there");
        assert!(
            Arc::ptr_eq(&object.mesh, &before),
            "the mesh is the one it had"
        );
    }
}
