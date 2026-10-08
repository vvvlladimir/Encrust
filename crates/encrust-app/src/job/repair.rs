use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use core_geometry::{
    Filled, Mesh, Orientation, fill_holes, orient_outward, remove_duplicate_faces,
    remove_unbalanced_faces,
};

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
    /// Faces that were lying over another face and have gone.
    pub duplicates_removed: usize,
    /// Faces at an edge the surface could not be wound round, which have gone with it.
    pub tangles_removed: usize,
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

/// Drops the faces drawn twice and the ones nothing can be wound round, patches the holes
/// both leave, and turns the patches the right way out.
///
/// The duplicates go first: an edge they double makes the surface look as if it branched
/// there, and a boundary cannot be followed through it. Winding is made as consistent as
/// the mesh allows before the tangles are judged, or a shell written inside out would read
/// as one tangle per edge. The orientation pass is last because a patch is new surface and
/// nothing has decided its side yet.
fn run(request: RepairRequest) -> RepairOutcome {
    let mut mesh = (*request.mesh).clone();
    let duplicates_removed = remove_duplicate_faces(&mut mesh);
    orient_outward(&mut mesh);
    let tangles_removed = remove_unbalanced_faces(&mut mesh);
    let filled = fill_holes(&mut mesh);
    let orientation = orient_outward(&mut mesh);
    RepairOutcome::Done(Box::new(Repaired {
        id: request.id,
        name: request.name,
        mesh: Arc::new(mesh),
        filled,
        duplicates_removed,
        tangles_removed,
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
    fn a_face_drawn_twice_beside_a_hole_comes_back_sound() {
        // What a broken exporter leaves: one triangle of a face missing, and one of the
        // triangles written twice, which makes three edges look as if they branched.
        let mut mesh = open_box();
        mesh.faces.push([1, 2, 6]);
        assert!(!diagnose(&mesh).is_sound());

        let mut job = RepairJob::spawn(RepairRequest {
            id: ObjectId::for_test(3),
            name: "broken.stl".to_owned(),
            mesh: Arc::new(mesh),
        });
        let outcome = loop {
            if let Some(outcome) = job.poll() {
                break outcome;
            }
            std::thread::yield_now();
        };

        let RepairOutcome::Done(repaired) = outcome else {
            panic!("the box has both to repair");
        };
        assert_eq!(repaired.duplicates_removed, 1);
        assert_eq!(repaired.filled.loops_filled, 1);
        assert!(
            diagnose(&repaired.mesh).is_sound(),
            "nothing is left for the window to call it broken for"
        );
    }

    /// Two of those boxes meeting along one vertical edge, the near one open at the top:
    /// four faces use the shared edge, twice each way, which is a seam rather than a hole.
    fn two_boxes_sharing_an_edge() -> Mesh {
        let mut mesh = open_box();
        let other = core_geometry::transform_mesh(
            &closed_box(),
            core_geometry::Transform::from_translation(Vec3::new(1.0, 1.0, 0.0)),
        );
        let offset = mesh.vertices.len() as u32;
        mesh.vertices.extend(other.vertices);
        mesh.faces.extend(
            other
                .faces
                .iter()
                .map(|face| face.map(|index| index + offset)),
        );
        core_geometry::weld(&mesh, core_geometry::DEFAULT_WELD_TOLERANCE).mesh
    }

    /// The box of [`open_box`] with its top on.
    fn closed_box() -> Mesh {
        let mut mesh = open_box();
        mesh.faces.push([4, 5, 6]);
        mesh.faces.push([4, 6, 7]);
        mesh
    }

    #[test]
    fn a_seam_two_solids_share_is_not_something_to_repair() {
        let mesh = two_boxes_sharing_an_edge();
        assert_eq!(diagnose(&mesh).non_manifold_edges, 1, "the shared edge");

        let mut job = RepairJob::spawn(RepairRequest {
            id: ObjectId::for_test(5),
            name: "pair.stl".to_owned(),
            mesh: Arc::new(mesh),
        });
        let outcome = loop {
            if let Some(outcome) = job.poll() {
                break outcome;
            }
            std::thread::yield_now();
        };

        let RepairOutcome::Done(repaired) = outcome else {
            panic!("the open box has a hole to close");
        };
        assert_eq!(repaired.tangles_removed, 0, "a seam costs no face");
        assert_eq!(repaired.filled.loops_filled, 1);
        assert!(
            diagnose(&repaired.mesh).is_sound(),
            "closing the hole leaves nothing to call it broken for"
        );
    }

    #[test]
    fn a_face_nothing_can_be_wound_round_is_dropped_and_the_model_comes_back_sound() {
        // A fin standing on one edge of the box, which is what a boolean leaves behind:
        // no winding covers the three faces that edge now has.
        let mut mesh = closed_box();
        mesh.vertices.push(Vec3::new(0.5, 0.5, 2.0));
        mesh.faces.push([4, 5, 8]);

        let mut job = RepairJob::spawn(RepairRequest {
            id: ObjectId::for_test(9),
            name: "tangled.stl".to_owned(),
            mesh: Arc::new(mesh),
        });
        let outcome = loop {
            if let Some(outcome) = job.poll() {
                break outcome;
            }
            std::thread::yield_now();
        };

        let RepairOutcome::Done(repaired) = outcome else {
            panic!("the sliver is something to repair");
        };
        assert!(repaired.tangles_removed > 0);
        assert!(diagnose(&repaired.mesh).is_sound());
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
