use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use core_geometry::{Mesh, Quat};
use core_plate::{OrientSettings, orient};
use rayon::prelude::*;

use crate::job::pipeline::{thread_pool, worker_threads};
use crate::scene::{ObjectId, Scene};

/// One model to find an orientation for, owned so that it can cross to the worker thread.
pub struct OrientTask {
    pub id: ObjectId,
    /// The model in its own space. Orientation is measured there, so where it currently
    /// stands does not enter into it.
    pub mesh: Arc<Mesh>,
}

/// Everything one orientation run needs.
pub struct OrientRequest {
    pub tasks: Vec<OrientTask>,
    pub settings: OrientSettings,
}

/// How a model should be turned, and what it will cost the print once it is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Turn {
    pub id: ObjectId,
    pub rotation: Quat,
    /// Degrees the model turns from where it stood, so the status bar can say whether
    /// anything moved.
    pub angle_deg: f32,
}

/// How an orientation run ended.
#[derive(Debug, Clone, PartialEq)]
pub enum OrientOutcome {
    Turned(Vec<Turn>),
    /// The whole error chain flattened into one line, as the window has no terminal.
    Failed(String),
}

/// An orientation run on its own thread.
///
/// Every candidate is measured over the whole mesh and the best few are sliced, which is
/// seconds on a detailed model and far too much for a frame.
#[derive(Debug)]
pub struct OrientJob {
    outcome: Receiver<OrientOutcome>,
}

impl OrientJob {
    /// Starts the run. The thread is detached: it ends on its own.
    pub fn spawn(request: OrientRequest) -> Self {
        let (sender, outcome) = mpsc::channel();
        crate::job::spawn(move || {
            let _ = sender.send(run(&request));
        });
        Self { outcome }
    }

    /// Returns the outcome on the frame the run ends, and `None` while it is going.
    pub fn poll(&mut self) -> Option<OrientOutcome> {
        match self.outcome.try_recv() {
            Ok(outcome) => Some(outcome),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(OrientOutcome::Failed(
                "the orientation thread stopped without finishing".to_owned(),
            )),
        }
    }
}

/// The models a run covers, each in its own space: one when `only` names it, everything
/// on the plate being edited otherwise.
pub fn orient_tasks(scene: &Scene, only: Option<ObjectId>) -> Vec<OrientTask> {
    scene
        .targets()
        .filter(|object| only.is_none_or(|id| object.id == id))
        .map(|object| OrientTask {
            id: object.id,
            mesh: Arc::clone(&object.mesh),
        })
        .collect()
}

fn run(request: &OrientRequest) -> OrientOutcome {
    let pool = match thread_pool(worker_threads()) {
        Ok(pool) => pool,
        Err(error) => return OrientOutcome::Failed(error.to_string()),
    };

    pool.install(|| {
        let turned: Result<Vec<Turn>, _> = request
            .tasks
            .par_iter()
            .map(|task| {
                orient(&task.mesh, &request.settings).map(|found| Turn {
                    id: task.id,
                    rotation: found.rotation,
                    angle_deg: found.rotation.to_axis_angle().1.to_degrees(),
                })
            })
            .collect();

        match turned {
            Ok(turns) => OrientOutcome::Turned(turns),
            Err(error) => OrientOutcome::Failed(error.to_string()),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::Vec3;

    /// Axis-aligned box, twelve triangles, spanning 0..size on every axis.
    fn cuboid(size: Vec3) -> Mesh {
        let vertices = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(size.x, 0.0, 0.0),
            Vec3::new(size.x, size.y, 0.0),
            Vec3::new(0.0, size.y, 0.0),
            Vec3::new(0.0, 0.0, size.z),
            Vec3::new(size.x, 0.0, size.z),
            Vec3::new(size.x, size.y, size.z),
            Vec3::new(0.0, size.y, size.z),
        ];
        let faces = vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        Mesh::new(vertices, faces)
    }

    #[test]
    fn a_run_reports_a_turn_for_every_model_it_was_given() {
        let request = OrientRequest {
            tasks: vec![
                OrientTask {
                    id: ObjectId::for_test(1),
                    mesh: Arc::new(cuboid(Vec3::new(40.0, 40.0, 4.0))),
                },
                OrientTask {
                    id: ObjectId::for_test(2),
                    mesh: Arc::new(cuboid(Vec3::splat(10.0))),
                },
            ],
            settings: OrientSettings::default(),
        };

        let OrientOutcome::Turned(turns) = run(&request) else {
            panic!("two sound models can be oriented");
        };
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].id, ObjectId::for_test(1));
    }

    #[test]
    fn a_model_with_no_geometry_fails_the_run() {
        let request = OrientRequest {
            tasks: vec![OrientTask {
                id: ObjectId::for_test(1),
                mesh: Arc::new(Mesh::default()),
            }],
            settings: OrientSettings::default(),
        };

        let OrientOutcome::Failed(message) = run(&request) else {
            panic!("an empty mesh has no orientation");
        };
        assert!(message.contains("orientation"), "got {message}");
    }
}
