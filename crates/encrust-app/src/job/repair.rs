use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use core_geometry::{
    Filled, Mesh, MeshDiagnostics, Orientation, diagnose, fill_holes, orient_outward,
    remove_duplicate_faces,
};

use crate::i18n::tr;
use crate::scene::{ObjectId, Shaped};

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
    pub shaped: Shaped,
    pub filled: Filled,
    /// Faces that were lying over another face and have gone.
    pub duplicates_removed: usize,
    /// What the patch had to be turned round, since a new face can come out either way.
    pub orientation: Orientation,
    /// The repair would have left more wrong than it found, so the mesh is as it came.
    pub kept_as_found: bool,
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
    id: ObjectId,
    name: String,
}

impl RepairJob {
    /// Starts the repair. The thread is detached: it ends on its own.
    pub fn spawn(request: RepairRequest) -> Self {
        let (sender, outcome) = mpsc::channel();
        let (id, name) = (request.id, request.name.clone());
        crate::job::spawn(move || {
            let _ = sender.send(run(request));
        });
        Self { outcome, id, name }
    }

    /// Returns the outcome on the frame the repair ends, and `None` while it is going.
    pub fn poll(&mut self) -> Option<RepairOutcome> {
        match self.outcome.try_recv() {
            Ok(outcome) => Some(outcome),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(RepairOutcome::Failed(tr!(
                "job-thread-stopped-repair",
                name = self.name.clone()
            ))),
        }
    }

    pub fn label(&self) -> String {
        tr!("job-repairing", name = self.name.clone())
    }

    /// The model being repaired.
    pub fn id(&self) -> ObjectId {
        self.id
    }
}

fn run(request: RepairRequest) -> RepairOutcome {
    let mut mesh = (*request.mesh).clone();
    let mended = mend(&mut mesh);
    RepairOutcome::Done(Box::new(Repaired {
        id: request.id,
        name: request.name,
        shaped: Shaped::of(Arc::new(mesh)),
        filled: mended.filled,
        duplicates_removed: mended.duplicates_removed,
        orientation: mended.orientation,
        kept_as_found: mended.kept_as_found,
    }))
}

/// What mending one mesh changed.
#[derive(Debug, Clone, PartialEq)]
pub struct Mended {
    pub filled: Filled,
    pub duplicates_removed: usize,
    pub orientation: Orientation,
    pub kept_as_found: bool,
}

/// Patches the holes and drops the faces laid over others, and turns the patches the
/// right way out; leaves the mesh as it came if that would leave more wrong with it, or
/// nothing of it at all.
///
/// The doubled faces go first, since one reads as a branch the boundary walk cannot get
/// through, and again after the filling: a patch over a lone face or a fin is its reverse,
/// and the two cancel. The orientation pass is last because a patch is new surface and
/// nothing has decided its side yet. See `docs/design/mesh-repair.md`.
pub fn mend(mesh: &mut Mesh) -> Mended {
    let found = mesh.clone();
    let before = wrong(&diagnose(mesh));

    let mut duplicates_removed = remove_duplicate_faces(mesh).len();
    orient_outward(mesh);
    let filled = fill_holes(mesh);
    duplicates_removed += remove_duplicate_faces(mesh).len();
    let orientation = orient_outward(mesh);

    // A sheet closed by its own reverse cancels away; a model that is nothing else would
    // vanish from the plate rather than be repaired.
    let kept_as_found = mesh.faces.is_empty() || wrong(&diagnose(mesh)) > before;
    if kept_as_found {
        *mesh = found;
    }
    Mended {
        filled,
        duplicates_removed,
        orientation,
        kept_as_found,
    }
}

/// Everything that keeps a mesh from being sound, counted together.
fn wrong(diagnostics: &MeshDiagnostics) -> usize {
    diagnostics.boundary_edges + diagnostics.degenerate_faces + diagnostics.duplicate_faces
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
        assert!(diagnose(&repaired.shaped.mesh).is_closed());
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
            diagnose(&repaired.shaped.mesh).is_sound(),
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
        assert_eq!(repaired.duplicates_removed, 0, "a seam costs no face");
        assert!(!repaired.kept_as_found);
        assert_eq!(repaired.filled.loops_filled, 1);
        assert!(
            diagnose(&repaired.shaped.mesh).is_sound(),
            "closing the hole leaves nothing to call it broken for"
        );
    }

    #[test]
    fn a_fin_is_cancelled_by_its_patch_and_the_model_comes_back_as_it_was() {
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
            panic!("the fin is something to repair");
        };
        assert_eq!(repaired.duplicates_removed, 2, "the fin and its reverse");
        assert_eq!(repaired.shaped.mesh.faces, closed_box().faces);
        assert!(diagnose(&repaired.shaped.mesh).is_sound());
    }

    #[test]
    fn a_model_that_is_one_sheet_is_kept_rather_than_cancelled_away() {
        let mut mesh = Mesh::new(vec![Vec3::ZERO, Vec3::X, Vec3::Y], vec![[0, 1, 2]]);
        let mended = mend(&mut mesh);

        assert!(mended.kept_as_found);
        assert_eq!(mesh.faces, vec![[0, 1, 2]]);
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
