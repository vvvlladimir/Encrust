//! What the Repair tool's Faces mode holds: the faces and open edges picked on one model,
//! and the changes by hand under way.

use std::sync::Arc;

use core_geometry::{Adjacency, Mesh, Region, Vec3};

use crate::highlight::{EdgeMark, FaceMark, Highlight, LIFT_MM, patch};
use crate::i18n::tr;
use crate::job::{Edit, EditJob, EditOutcome};
use crate::scene::{ObjectId, Scene, SceneObject, Taken};
use crate::status::Status;
use crate::ui::theme;

/// The faces picked on one model, and the patch they are marked with.
#[derive(Debug)]
struct Faces {
    taken: Taken,
    region: Region,
    patch: Option<Arc<Mesh>>,
}

/// The picks and the changes by hand under way.
#[derive(Debug, Default)]
pub struct Hand {
    faces: Option<Faces>,
    /// The open edges picked, at most two, on the one model they were picked on.
    edges: Option<(Taken, Vec<[u32; 2]>)>,
    jobs: Vec<(EditJob, Taken)>,
}

/// A change by hand the Faces mode's foot asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandEdit {
    Flip,
    Remove,
    Bridge,
}

impl Hand {
    /// Marks the faces of `object` within `radius_mm` of `at` on the plate, or unmarks them.
    pub fn brush(&mut self, object: &SceneObject, at: Vec3, radius_mm: f32, marked: bool) {
        let model = (&*object.mesh, &*object.bvh, object.transform);
        self.faces_of(object)
            .region
            .brush(model, at, radius_mm, marked);
    }

    /// Marks the surface of `object` round `seed` that turns no more than `angle_deg`, or
    /// unmarks it.
    pub fn flood(&mut self, object: &SceneObject, seed: usize, angle_deg: f32, marked: bool) {
        let adjacency = Adjacency::of(&object.mesh);
        let faces = self.faces_of(object);
        faces
            .region
            .flood(&object.mesh, &adjacency, seed, angle_deg, marked);
    }

    /// Lays the picked faces' patch again, once a stroke's frame has changed them.
    pub fn refresh(&mut self, scene: &Scene) {
        let Some(faces) = self.faces.as_mut() else {
            return;
        };
        faces.patch = scene
            .get(faces.taken.id)
            .and_then(|object| patch(&object.mesh, faces.region.faces(), LIFT_MM));
    }

    /// The model the faces are picked on, and how many, while it is as they were picked on.
    pub fn picked_faces<'a>(&self, scene: &'a Scene) -> Option<(&'a SceneObject, usize)> {
        let faces = self.faces.as_ref()?;
        let object = scene
            .get(faces.taken.id)
            .filter(|object| faces.taken.is_model(object))?;
        (!faces.region.is_empty()).then(|| (object, faces.region.len()))
    }

    pub fn clear(&mut self) {
        (self.faces, self.edges) = (None, None);
    }

    /// Picks the open edge `edge` of `object`, or lets it go if picked; a third pick lets
    /// the first go.
    pub fn pick_edge(&mut self, object: &SceneObject, edge: [u32; 2]) {
        let fresh = self
            .edges
            .as_ref()
            .is_none_or(|(taken, _)| !taken.is_model(object));
        if fresh {
            self.edges = Some((Taken::of(object), Vec::new()));
        }
        let Some((_, edges)) = self.edges.as_mut() else {
            return;
        };
        let same = |picked: &[u32; 2]| picked == &edge || picked == &[edge[1], edge[0]];
        match edges.iter().position(same) {
            Some(at) => {
                edges.remove(at);
            }
            None => {
                if edges.len() == 2 {
                    edges.remove(0);
                }
                edges.push(edge);
            }
        }
    }

    /// The open edges picked on the model as it stands.
    pub fn picked_edges<'a>(&'a self, scene: &Scene) -> &'a [[u32; 2]] {
        match &self.edges {
            Some((taken, edges)) if scene.get(taken.id).is_some_and(|o| taken.is_model(o)) => edges,
            _ => &[],
        }
    }

    /// Starts `edit` on what is picked, and answers whether there was anything to start it on.
    pub fn start(&mut self, scene: &Scene, edit: HandEdit, status: &mut Status) -> bool {
        let asked = match edit {
            HandEdit::Flip | HandEdit::Remove => self.picked_faces(scene).map(|(object, _)| {
                let region = self
                    .faces
                    .take()
                    .map(|faces| faces.region)
                    .unwrap_or_default();
                let edit = match edit {
                    HandEdit::Flip => Edit::Flip(region),
                    _ => Edit::Remove(region),
                };
                (object, edit)
            }),
            HandEdit::Bridge => match (self.picked_edges(scene), &self.edges) {
                ([a, b], Some((taken, _))) => {
                    scene.get(taken.id).map(|o| (o, Edit::Bridge(*a, *b)))
                }
                _ => None,
            },
        };
        let Some((object, edit)) = asked else {
            return false;
        };
        if matches!(edit, Edit::Bridge(..)) {
            self.edges = None;
        }
        self.run(object, edit, status);
        true
    }

    /// Drops the shells of `id` numbered `which` in `list_shells` order.
    pub fn drop_shells(
        &mut self,
        scene: &Scene,
        id: ObjectId,
        which: Vec<usize>,
        status: &mut Status,
    ) {
        if let Some(object) = scene.get(id).filter(|_| !which.is_empty()) {
            self.run(object, Edit::DropShells(which), status);
        }
    }

    fn run(&mut self, object: &SceneObject, edit: Edit, status: &mut Status) {
        let job = EditJob::spawn(Arc::clone(&object.mesh), object.name.clone(), edit);
        *status = Status::Info(tr!("edit-editing", name = object.name.clone()));
        self.jobs.push((job, Taken::of(object)));
    }

    pub fn is_busy(&self) -> bool {
        !self.jobs.is_empty()
    }

    /// Puts every change that has finished on its model, unless the model changed since.
    /// Returns whether one is still running.
    pub fn poll(&mut self, scene: &mut Scene, status: &mut Status) -> bool {
        let mut done = Vec::new();
        self.jobs.retain_mut(|(job, taken)| match job.poll() {
            None => true,
            Some(outcome) => {
                done.push((taken.clone(), outcome));
                false
            }
        });
        for (taken, outcome) in done {
            *status = match (outcome, scene.get_mut(taken.id)) {
                (EditOutcome::Failed(message), _) => Status::Error(message),
                (EditOutcome::Done(shaped, said), Some(object)) if taken.is_model(object) => {
                    object.reshape(*shaped);
                    Status::Info(said)
                }
                (EditOutcome::Done(..), _) => {
                    Status::Error(tr!("job-model-changed", name = taken.name))
                }
            };
        }
        self.is_busy()
    }

    /// The picked faces and edges, marked in `fault` over the models.
    pub fn highlight(&self, scene: &Scene) -> Highlight {
        let colour = theme::scene().fault;
        let mut highlight = Highlight::default();
        if let (Some(faces), Some((object, _))) = (&self.faces, self.picked_faces(scene))
            && let Some(patch) = &faces.patch
        {
            highlight.faces.push(FaceMark {
                mesh: Arc::clone(patch),
                transform: object.transform,
                colour,
            });
        }
        if let Some((taken, _)) = &self.edges
            && let Some(object) = scene.get(taken.id)
        {
            let matrix = object.transform.to_matrix();
            let at = |corner: u32| matrix.transform_point3(object.mesh.vertices[corner as usize]);
            highlight.edges.push(EdgeMark {
                edges: self
                    .picked_edges(scene)
                    .iter()
                    .map(|edge| edge.map(at))
                    .collect(),
                colour,
            });
        }
        highlight
    }

    /// The faces picked on `object`, begun afresh on a model other than the last one.
    fn faces_of(&mut self, object: &SceneObject) -> &mut Faces {
        let faces = match self.faces.take() {
            Some(faces) if faces.taken.is_model(object) => faces,
            _ => Faces {
                taken: Taken::of(object),
                region: Region::default(),
                patch: None,
            },
        };
        self.faces.insert(faces)
    }
}

#[cfg(test)]
mod tests;
