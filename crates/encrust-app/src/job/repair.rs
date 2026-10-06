use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use core_geometry::{Filled, Mesh, Orientation, fill_holes, orient_outward};

use crate::scene::ObjectId;

/// One model to close the holes in, owned so that it can cross to the worker thread.
pub struct RepairRequest {
    pub id: ObjectId,
    pub name: String,
    pub mesh: Arc<Mesh>,
}

/// One model as repair left it, in the space it was in.
#[derive(Debug, Clone, PartialEq)]
pub struct Repaired {
    pub id: ObjectId,
    pub name: String,
    pub mesh: Arc<Mesh>,
    pub filled: Filled,
    /// What the patch had to be turned round, since a new face can come out either way.
    pub orientation: Orientation,
}

/// How one repair ended.
#[derive(Debug, Clone, PartialEq)]
pub enum RepairOutcome {
    /// Boxed because it carries a whole model, like an import's own outcome.
    Done(Box<Repaired>),
    /// The whole error chain flattened into one line, as the window has no terminal.
    Failed(String),
}

/// One model being repaired on its own thread.
///
/// Closing the holes walks the mesh's edges and rebuilds its hierarchy afterwards, which
/// is far too much for a frame on a real part; see `docs/decisions/0038`.
#[derive(Debug)]
pub struct RepairJob {
    outcome: Receiver<RepairOutcome>,
    name: String,
}

impl RepairJob {
    /// Starts the repair. The thread is detached: it ends on its own.
    pub fn spawn(request: RepairRequest) -> Self {
        let (sender, outcome) = mpsc::channel();
        let name = request.name.clone();
        crate::job::spawn(move || {
            let _ = sender.send(run(request));
        });
        Self { outcome, name }
    }

    /// Returns the outcome on the frame the repair ends, and `None` while it is going.
    pub fn poll(&mut self) -> Option<RepairOutcome> {
        match self.outcome.try_recv() {
            Ok(outcome) => Some(outcome),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(RepairOutcome::Failed(format!(
                "the thread repairing {} stopped without finishing",
                self.name
            ))),
        }
    }

    pub fn label(&self) -> String {
        format!("Repairing {}", self.name)
    }
}

/// Patches the holes and turns the patches the right way out. The orientation pass is run
/// again because a patch is new surface, and nothing has decided its side yet.
fn run(request: RepairRequest) -> RepairOutcome {
    let mut mesh = (*request.mesh).clone();
    let filled = fill_holes(&mut mesh);
    let orientation = orient_outward(&mut mesh);
    RepairOutcome::Done(Box::new(Repaired {
        id: request.id,
        name: request.name,
        mesh: Arc::new(mesh),
        filled,
        orientation,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::{Vec3, diagnose};

    /// A box of ten triangles: a cube with the two of its top missing.
    fn open_box() -> Mesh {
        Mesh::new(
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
        )
    }

    #[test]
    fn a_repaired_model_comes_back_closed() {
        let mut job = RepairJob::spawn(RepairRequest {
            id: ObjectId::for_test(7),
            name: "open.stl".to_owned(),
            mesh: Arc::new(open_box()),
        });
        let outcome = loop {
            if let Some(outcome) = job.poll() {
                break outcome;
            }
            std::thread::yield_now();
        };

        let RepairOutcome::Done(repaired) = outcome else {
            panic!("an open box has a hole to close");
        };
        assert_eq!(repaired.id, ObjectId::for_test(7));
        assert_eq!(repaired.filled.loops_filled, 1);
        assert!(diagnose(&repaired.mesh).is_closed());
        assert!(repaired.orientation.orientable);
    }

    #[test]
    fn a_running_repair_says_which_model_it_is_on() {
        let job = RepairJob::spawn(RepairRequest {
            id: ObjectId::for_test(1),
            name: "dragon.stl".to_owned(),
            mesh: Arc::new(open_box()),
        });
        assert_eq!(job.label(), "Repairing dragon.stl");
    }
}
