//! Where the Repair tool's findings lie on the models, worked out off the drawing thread,
//! marked over them while the tool is open, and visited one place at a time.

use std::sync::Arc;

use core_geometry::{FastMap, FastSet, Faults, Mesh, Region, Vec3, faults, self_intersections};

use crate::highlight::{EdgeMark, FaceMark, Highlight, LIFT_MM, Spot, patch};
use crate::job::{Worker, pooled};
use crate::scene::{ObjectId, Scene, SceneObject, Taken};
use crate::ui::theme;

/// Which kind of finding the viewport marks on the models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Finding {
    /// The holes, by their rims: every edge one face uses.
    OpenEdges,
    FlatFaces,
    DoubledFaces,
    CrossingFaces,
}

/// One hole, in its model's space: where it is, and the edges round it.
#[derive(Debug)]
struct Hole {
    spot: Spot,
    rim: Vec<[Vec3; 2]>,
}

/// Faces of one kind in a model: all of them in one patch, and each piece of them in its
/// own, stood further off so that the piece in view is drawn over the rest.
#[derive(Debug, Default)]
struct Faces {
    all: Option<Arc<Mesh>>,
    parts: Vec<(Spot, Arc<Mesh>)>,
}

/// Where one model's faults lie, in its own space.
#[derive(Debug, Default)]
struct Placed {
    open_edges: Vec<[Vec3; 2]>,
    holes: Vec<Hole>,
    flat_faces: Faces,
    doubled_faces: Faces,
}

/// One model's faces crossing others: how many pairs, and the faces.
#[derive(Debug)]
struct Crossed {
    pairs: usize,
    faces: Faces,
}

/// A piece of work on one model: running, or done with an answer or without one.
#[derive(Debug)]
enum Pending<T> {
    Running(Worker<Option<T>>),
    Done(Option<T>),
}

/// The findings worked out on each model, kept while its mesh is the one they were of,
/// and the place among them the camera was sent to.
#[derive(Debug, Default)]
pub struct Found {
    shown: Option<Finding>,
    /// Which of the shown finding's places is in view, counted over the targets in order.
    at: usize,
    /// Whether the camera is still to go there, once the places are worked out.
    flying: bool,
    placed: Vec<(Taken, Pending<Placed>)>,
    crossed: Vec<(Taken, Pending<Crossed>)>,
}

impl Found {
    /// The kind marked on the models while the Repair tool is open.
    pub fn shown(&self) -> Option<Finding> {
        self.shown
    }

    /// A press on a finding's row: the first shows it and goes to its first place, each
    /// after goes to the next, and the one after the last puts the marks away.
    pub fn press(&mut self, scene: &Scene, finding: Finding) {
        if self.shown != Some(finding) {
            (self.shown, self.at, self.flying) = (Some(finding), 0, true);
            return;
        }
        // Until the places are worked out there is no next one to go to.
        let Some(count) = self.spots(scene, finding).map(|spots| spots.len()) else {
            return;
        };
        if self.at + 1 >= count {
            (self.shown, self.at, self.flying) = (None, 0, false);
        } else {
            (self.at, self.flying) = (self.at + 1, true);
        }
    }

    /// The place the camera is to go to, once on the frame it is worked out.
    pub fn take_flight(&mut self, scene: &Scene) -> Option<Spot> {
        if !self.flying {
            return None;
        }
        let spots = self.spots(scene, self.shown?)?;
        self.flying = false;
        spots.get(self.at).copied()
    }

    /// Starts counting the crossing faces of every model in `ids` not counted as it stands.
    pub fn count_crossings(&mut self, scene: &Scene, ids: &[ObjectId]) {
        for object in ids.iter().filter_map(|id| scene.get(*id)) {
            let (mesh, bvh) = (Arc::clone(&object.mesh), Arc::clone(&object.bvh));
            start(&mut self.crossed, object, move || {
                let pairs = self_intersections(&mesh, &bvh);
                let mut faces = Region::default();
                for face in pairs.iter().flatten() {
                    faces.set(*face as usize, true);
                }
                Crossed {
                    pairs: pairs.len(),
                    faces: Faces::of(&mesh, &faces),
                }
            });
        }
    }

    /// The crossing pairs counted on the model as it stands: `None` while uncounted,
    /// `Some(None)` while counting or where the count gave no answer.
    pub fn crossings_of(&self, object: &SceneObject) -> Option<Option<usize>> {
        done(&self.crossed, object).map(|crossed| crossed.map(|crossed| crossed.pairs))
    }

    /// Starts placing the faults of every model in `ids` not placed as it stands.
    pub fn place(&mut self, scene: &Scene, ids: &[ObjectId]) {
        for object in ids.iter().filter_map(|id| scene.get(*id)) {
            let mesh = Arc::clone(&object.mesh);
            start(&mut self.placed, object, move || placed(&mesh));
        }
    }

    /// Whether some work is still running.
    pub fn is_working(&self) -> bool {
        running(&self.placed) || running(&self.crossed)
    }

    /// Takes in what has finished, and forgets what was of a mesh replaced since. Returns
    /// whether some work is still running.
    pub fn poll(&mut self, scene: &Scene) -> bool {
        settle(&mut self.placed, scene);
        settle(&mut self.crossed, scene);
        self.is_working()
    }

    /// The places of `finding` on the plate over the targets in order, or `None` until
    /// every target's are worked out.
    fn spots(&self, scene: &Scene, finding: Finding) -> Option<Vec<Spot>> {
        let mut spots = Vec::new();
        for object in scene.targets() {
            let local = match finding {
                Finding::CrossingFaces => done(&self.crossed, object)?
                    .map(|crossed| crossed.faces.spots())
                    .unwrap_or_default(),
                _ => done(&self.placed, object)?
                    .map(|placed| placed.spots(finding))
                    .unwrap_or_default(),
            };
            spots.extend(local.into_iter().map(|spot| spot.placed(object.transform)));
        }
        Some(spots)
    }

    /// The shown finding marked on the targets as far as it is worked out: the place in
    /// view in `fault`, the rest in `fault_rest`.
    pub fn highlight(&self, scene: &Scene) -> Highlight {
        let mut highlight = Highlight::default();
        let Some(shown) = self.shown else {
            return highlight;
        };
        let mut marking = Marking {
            highlight: &mut highlight,
            focused: Vec::new(),
            at: self.at,
            first: 0,
        };
        for object in scene.targets() {
            match shown {
                Finding::OpenEdges => {
                    if let Some(placed) = done(&self.placed, object).flatten() {
                        marking.holes(object, placed);
                    }
                }
                Finding::CrossingFaces => {
                    if let Some(crossed) = done(&self.crossed, object).flatten() {
                        marking.faces(object, &crossed.faces);
                    }
                }
                Finding::FlatFaces | Finding::DoubledFaces => {
                    if let Some(placed) = done(&self.placed, object).flatten() {
                        let faces = match shown {
                            Finding::FlatFaces => &placed.flat_faces,
                            _ => &placed.doubled_faces,
                        };
                        marking.faces(object, faces);
                    }
                }
            }
        }
        // Last, so that the rim in view is drawn over the same edges in the other colour.
        let Marking { focused, .. } = marking;
        highlight.edges.push(EdgeMark {
            edges: focused,
            colour: theme::scene().fault,
        });
        highlight
    }
}

/// A highlight being laid down over the targets in order, counting their places to find
/// the one in view.
struct Marking<'a> {
    highlight: &'a mut Highlight,
    /// The rim of the hole in view, on the plate.
    focused: Vec<[Vec3; 2]>,
    at: usize,
    /// The number of the first place of the model being marked.
    first: usize,
}

impl Marking<'_> {
    /// Which of `count` places of the model being marked is in view, if one is.
    fn local(&mut self, count: usize) -> Option<usize> {
        let local = self.at.checked_sub(self.first).filter(|&at| at < count);
        self.first += count;
        local
    }

    fn holes(&mut self, object: &SceneObject, placed: &Placed) {
        let matrix = object.transform.to_matrix();
        let on_plate = |edges: &[[Vec3; 2]]| -> Vec<[Vec3; 2]> {
            let place = |edge: &[Vec3; 2]| edge.map(|end| matrix.transform_point3(end));
            edges.iter().map(place).collect()
        };
        self.highlight.edges.push(EdgeMark {
            edges: on_plate(&placed.open_edges),
            colour: theme::scene().fault_rest,
        });
        if let Some(local) = self.local(placed.holes.len()) {
            self.focused.extend(on_plate(&placed.holes[local].rim));
        }
    }

    fn faces(&mut self, object: &SceneObject, faces: &Faces) {
        if let Some(all) = &faces.all {
            self.highlight.faces.push(FaceMark {
                mesh: Arc::clone(all),
                transform: object.transform,
                colour: theme::scene().fault_rest,
            });
        }
        if let Some(local) = self.local(faces.parts.len()) {
            self.highlight.faces.push(FaceMark {
                mesh: Arc::clone(&faces.parts[local].1),
                transform: object.transform,
                colour: theme::scene().fault,
            });
        }
    }
}

impl Placed {
    /// The places of `finding` in the model's own space.
    fn spots(&self, finding: Finding) -> Vec<Spot> {
        match finding {
            Finding::OpenEdges => self.holes.iter().map(|hole| hole.spot).collect(),
            Finding::FlatFaces => self.flat_faces.spots(),
            Finding::DoubledFaces => self.doubled_faces.spots(),
            Finding::CrossingFaces => Vec::new(),
        }
    }
}

impl Faces {
    /// The faces of `region` in `mesh`, in one patch and piece by piece, each piece seen
    /// from the side its faces face together.
    fn of(mesh: &Mesh, region: &Region) -> Self {
        let parts = region
            .parts(mesh)
            .into_iter()
            .filter_map(|faces| {
                let facing: Vec3 = faces
                    .iter()
                    .filter_map(|&face| mesh.triangle(face))
                    .map(|triangle| triangle.normal_unnormalized())
                    .sum();
                let corners = faces.iter().flat_map(|&face| mesh.faces[face]);
                let points = corners.map(|corner| mesh.vertices[corner as usize]);
                let spot = Spot::around(points, facing)?;
                Some((spot, patch(mesh, faces, 2.0 * LIFT_MM)?))
            })
            .collect();
        Self {
            all: patch(mesh, region.faces(), LIFT_MM),
            parts,
        }
    }

    fn spots(&self) -> Vec<Spot> {
        self.parts.iter().map(|(spot, _)| *spot).collect()
    }
}

/// The faults of `mesh` laid out as the viewport draws them and the camera visits them.
fn placed(mesh: &Mesh) -> Placed {
    let found = faults(mesh);
    let at = |corner: u32| mesh.vertices[corner as usize];
    Placed {
        open_edges: found.open_edges.iter().map(|edge| edge.map(at)).collect(),
        holes: holes(mesh, &found),
        flat_faces: Faces::of(mesh, &found.flat_faces),
        doubled_faces: Faces::of(mesh, &found.doubled_faces),
    }
}

/// Each hole with its rim, the open edges whose first end it holds. A loop is seen from
/// the side its patch would face; a branching rim from the side its faces face together.
fn holes(mesh: &Mesh, found: &Faults) -> Vec<Hole> {
    let at = |corner: u32| mesh.vertices[corner as usize];
    let open: FastSet<[u32; 2]> = found.open_edges.iter().copied().collect();
    let mut hole_of: FastMap<u32, usize> = FastMap::default();
    for (index, corners) in found.holes.iter().enumerate() {
        for corner in corners {
            hole_of.entry(*corner).or_insert(index);
        }
    }
    let mut rims: Vec<(Vec<[Vec3; 2]>, Vec3)> = vec![(Vec::new(), Vec3::ZERO); found.holes.len()];
    for (edge, face) in found.open_edges.iter().zip(&found.open_faces) {
        if let Some(&index) = hole_of.get(&edge[0]) {
            let (rim, facing) = &mut rims[index];
            rim.push(edge.map(at));
            if let Some(triangle) = mesh.triangle(*face as usize) {
                *facing += triangle.normal_unnormalized();
            }
        }
    }
    found
        .holes
        .iter()
        .zip(rims)
        .filter_map(|(corners, (rim, facing))| {
            let sides = || corners.iter().zip(corners.iter().cycle().skip(1));
            let looped = sides().all(|(&a, &b)| open.contains(&[a.min(b), a.max(b)]));
            let normal = match looped {
                true => sides().map(|(&a, &b)| at(a).cross(at(b))).sum(),
                false => facing,
            };
            let spot = Spot::around(corners.iter().map(|&corner| at(corner)), normal)?;
            Some(Hole { spot, rim })
        })
        .collect()
}

/// Starts `work` on `object` unless what is kept for it is of its mesh as it stands.
fn start<T: Send + 'static>(
    kept: &mut Vec<(Taken, Pending<T>)>,
    object: &SceneObject,
    work: impl FnOnce() -> T + Send + 'static,
) {
    if kept.iter().any(|(taken, _)| taken.is_model(object)) {
        return;
    }
    kept.retain(|(taken, _)| taken.id != object.id);
    let worker = Worker::spawn(move |_, _| pooled(|| Some(work()), |_| None), || None);
    kept.push((Taken::of(object), Pending::Running(worker)));
}

/// What is kept for `object` as it stands: `None` when nothing, `Some(None)` while it runs
/// or where it ended without an answer.
fn done<'a, T>(kept: &'a [(Taken, Pending<T>)], object: &SceneObject) -> Option<Option<&'a T>> {
    let (_, pending) = kept.iter().find(|(taken, _)| taken.is_model(object))?;
    Some(match pending {
        Pending::Running(_) => None,
        Pending::Done(answer) => answer.as_ref(),
    })
}

fn running<T>(kept: &[(Taken, Pending<T>)]) -> bool {
    kept.iter()
        .any(|(_, pending)| matches!(pending, Pending::Running(_)))
}

/// Takes in the answers that have come, and drops what was of a mesh no longer on the plate.
fn settle<T: Send + 'static>(kept: &mut Vec<(Taken, Pending<T>)>, scene: &Scene) {
    kept.retain_mut(|(taken, pending)| {
        if let Pending::Running(worker) = pending
            && let Some(answer) = worker.poll()
        {
            *pending = Pending::Done(answer);
        }
        scene
            .get(taken.id)
            .is_some_and(|object| taken.is_model(object))
    });
}

#[cfg(test)]
mod tests;
