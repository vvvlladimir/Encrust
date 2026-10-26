use std::sync::Arc;

use core_geometry::{BridgeError, Mesh, Region, bridge, flip_faces, remove_faces, remove_shells};

use crate::i18n::tr;
use crate::job::{Worker, pooled};
use crate::scene::Shaped;

/// A change made to one model's mesh by hand, the same calls the command line's flags make.
#[derive(Debug, Clone, PartialEq)]
pub enum Edit {
    Flip(Region),
    Remove(Region),
    /// Two open edges, each as its two vertices.
    Bridge([u32; 2], [u32; 2]),
    /// Shells numbered in `core_geometry::list_shells` order.
    DropShells(Vec<usize>),
}

/// How one change by hand ended.
#[derive(Debug, Clone, PartialEq)]
pub enum EditOutcome {
    /// The mesh as the change left it, and what the stage notice says it did.
    Done(Box<Shaped>, String),
    Failed(String),
}

/// One change by hand on its own thread: the hierarchy is built again after it, which on a
/// large model is longer than a frame.
#[derive(Debug)]
pub struct EditJob {
    worker: Worker<EditOutcome>,
}

impl EditJob {
    /// Starts `edit` on `mesh`, the model called `name`.
    pub fn spawn(mesh: Arc<Mesh>, name: String, edit: Edit) -> Self {
        let worker = Worker::spawn(
            move |_, _| pooled(|| run(&mesh, &name, edit), EditOutcome::Failed),
            || EditOutcome::Failed(tr!("job-thread-stopped-edit")),
        );
        Self { worker }
    }

    /// Returns the outcome on the frame the change ends, and `None` while it is going.
    pub fn poll(&mut self) -> Option<EditOutcome> {
        self.worker.poll()
    }
}

fn run(mesh: &Mesh, name: &str, edit: Edit) -> EditOutcome {
    let mut mesh = mesh.clone();
    let name = name.to_owned();
    let said = match edit {
        Edit::Flip(faces) => tr!(
            "edit-flipped",
            name = name,
            count = flip_faces(&mut mesh, &faces)
        ),
        Edit::Remove(faces) => {
            tr!(
                "edit-removed",
                name = name,
                count = remove_faces(&mut mesh, &faces)
            )
        }
        Edit::Bridge(a, b) => match bridge(&mut mesh, a, b) {
            Ok(count) => tr!("edit-bridged", name = name, count = count),
            Err(error) => return EditOutcome::Failed(refused(error)),
        },
        Edit::DropShells(which) => {
            tr!(
                "edit-dropped-shells",
                name = name,
                count = remove_shells(&mut mesh, &which)
            )
        }
    };
    EditOutcome::Done(Box::new(Shaped::of(Arc::new(mesh))), said)
}

/// Why a bridge was refused, in the words the stage notice says it in.
fn refused(error: BridgeError) -> String {
    match error {
        BridgeError::NotOpen(_) => tr!("edit-bridge-not-open"),
        BridgeError::SameEdge => tr!("edit-bridge-same"),
        BridgeError::Degenerate => tr!("edit-bridge-in-line"),
    }
}

#[cfg(test)]
mod tests {
    use core_geometry::{Vec3, cuboid, diagnose};

    use super::*;

    fn ended(edit: Edit, mesh: Mesh) -> EditOutcome {
        let mut job = EditJob::spawn(Arc::new(mesh), "cube.stl".to_owned(), edit);
        loop {
            if let Some(outcome) = job.poll() {
                return outcome;
            }
            std::thread::yield_now();
        }
    }

    fn picked(faces: &[usize]) -> Region {
        let mut region = Region::default();
        for face in faces {
            region.set(*face, true);
        }
        region
    }

    #[test]
    fn dropping_two_faces_by_hand_opens_the_cube_and_says_so() {
        let outcome = ended(Edit::Remove(picked(&[0, 1])), cuboid(Vec3::ZERO, Vec3::ONE));
        let EditOutcome::Done(shaped, said) = outcome else {
            panic!("dropping faces always lands");
        };
        assert_eq!(diagnose(&shaped.mesh).boundary_edges, 4);
        assert_eq!(said, tr!("edit-removed", name = "cube.stl", count = 2));
    }

    #[test]
    fn a_bridge_refused_says_why_in_words() {
        let cube = cuboid(Vec3::ZERO, Vec3::ONE);
        let [a, b, _] = cube.faces[0];
        let outcome = ended(Edit::Bridge([a, b], [b, a]), cube);
        assert_eq!(outcome, EditOutcome::Failed(tr!("edit-bridge-same")));
    }

    #[test]
    fn a_turned_face_lands_and_a_dropped_shell_takes_its_faces() {
        let mut two = cuboid(Vec3::ZERO, Vec3::ONE);
        two.append(&cuboid(Vec3::splat(5.0), Vec3::splat(6.0)));
        let EditOutcome::Done(shaped, _) = ended(Edit::DropShells(vec![1]), two) else {
            panic!("dropping a shell always lands");
        };
        assert_eq!(diagnose(&shaped.mesh).shells, 1);
        let EditOutcome::Done(_, said) =
            ended(Edit::Flip(picked(&[3])), cuboid(Vec3::ZERO, Vec3::ONE))
        else {
            panic!("turning a face always lands");
        };
        assert_eq!(said, tr!("edit-flipped", name = "cube.stl", count = 1));
    }
}
