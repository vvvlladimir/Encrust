//! What the window does about a model that arrived broken: the runs that close the holes,
//! started from the model's own row.
//!
//! Welding, orientation and dropping the faces laid over others are done on import without
//! asking, because none of them changes what the model encloses (ADR 0005, 0268). Closing a
//! hole invents surface that was never in the file, so it waits for the Repair button; see
//! `docs/decisions/0268-repair-cancels-what-encloses-nothing-and-never-worsens.md`.

use std::sync::Arc;

use crate::job::{RepairJob, RepairOutcome, RepairRequest, Repaired};
use crate::scene::{ObjectId, Scene};
use crate::status::Status;

/// The repairs under way.
#[derive(Debug, Default)]
pub struct Repairs {
    jobs: Vec<RepairJob>,
}

impl Repairs {
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
    if repaired.kept_as_found {
        return Status::Error(format!(
            "{} could not be repaired without leaving more wrong with it, so it is as it came",
            repaired.name
        ));
    }
    object.reshape(repaired.shaped.clone());
    // What repair did is said in the stage notice; the summary keeps describing the file,
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

/// What it changed, in the words the stage notice says it in.
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
