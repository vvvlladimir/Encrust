//! The shells of the one model the Repair tool works on: listed off the drawing thread,
//! picked from the list, flown to and marked, hidden from view, and dropped.

use std::sync::Arc;

use core_geometry::{FastSet, Mesh, Vec3, list_shells};

use super::pending::{Pending, done, running, settle, start};
use crate::highlight::{FaceMark, Highlight, LIFT_MM, Spot, patch};
use crate::scene::{ObjectId, Scene, SceneObject, Taken};
use crate::ui::theme;

/// The share of the largest shell's area under which a shell is small: a loose bit beside
/// the model rather than a part of it.
const SMALL_SHARE: f32 = 0.01;

/// Faces past which a picked shell is flown to but not marked: the patch is laid on the
/// drawing thread, and a shell that size is the model itself.
const MARKED_FACES: usize = 100_000;

/// One shell as the list shows it, in its model's space.
#[derive(Debug, Clone)]
pub struct ShellRow {
    pub faces: Vec<u32>,
    pub area_mm2: f32,
    pub closed: bool,
    spot: Option<Spot>,
}

/// The shells listed, those picked, those hidden, and the camera's next flight to one.
#[derive(Debug, Default)]
pub struct Shells {
    listed: Vec<(Taken, Pending<Vec<ShellRow>>)>,
    picked: Option<(Taken, Vec<usize>, Option<Arc<Mesh>>)>,
    hidden: Option<(Taken, Vec<usize>)>,
    /// The model drawn without its hidden shells, made off the drawing thread.
    stand_in: Vec<(Taken, Pending<Arc<Mesh>>)>,
    flight: Option<Spot>,
}

impl Shells {
    /// Starts listing the shells of `object`, unless they are listed as it stands.
    pub fn list(&mut self, object: &SceneObject) {
        let mesh = Arc::clone(&object.mesh);
        start(&mut self.listed, object, move || rows(&mesh));
    }

    /// The shells of `object` as it stands, once listed.
    pub fn rows(&self, object: &SceneObject) -> Option<&[ShellRow]> {
        done(&self.listed, object).flatten().map(Vec::as_slice)
    }

    /// The shells of `object` picked, by their place in the list.
    pub fn picked(&self, object: &SceneObject) -> &[usize] {
        match &self.picked {
            Some((taken, picked, _)) if taken.is_model(object) => picked,
            _ => &[],
        }
    }

    /// Picks the shell at `at` in the list of `object` and sends the camera to it, or lets
    /// it go if it was picked.
    pub fn toggle(&mut self, object: &SceneObject, at: usize) {
        let mut picked = self.picked(object).to_vec();
        match picked.iter().position(|&shell| shell == at) {
            Some(index) => {
                picked.remove(index);
            }
            None => {
                picked.push(at);
                self.flight = self.rows(object).and_then(|rows| rows.get(at)?.spot);
                self.flight = self.flight.map(|spot| spot.placed(object.transform));
            }
        }
        self.set(object, picked);
    }

    /// Picks the shells of `object` whose area is under a hundredth of the largest one's.
    pub fn pick_small(&mut self, object: &SceneObject) {
        let rows = self.rows(object).unwrap_or_default();
        let largest = rows.iter().map(|row| row.area_mm2).fold(0.0, f32::max);
        let small = self.matching(object, |row| row.area_mm2 < largest * SMALL_SHARE);
        self.set(object, small);
    }

    /// Picks every shell of `object` not picked, and lets the picked ones go.
    pub fn invert(&mut self, object: &SceneObject) {
        let picked: FastSet<usize> = self.picked(object).iter().copied().collect();
        let rest = self.matching(object, |_| true);
        self.set(
            object,
            rest.into_iter().filter(|at| !picked.contains(at)).collect(),
        );
    }

    /// The shells of `object` left out of the view, by their place in the list.
    pub fn hidden(&self, object: &SceneObject) -> &[usize] {
        match &self.hidden {
            Some((taken, hidden)) if taken.is_model(object) => hidden,
            _ => &[],
        }
    }

    /// Hides the shell at `at` in the list of `object` from the view, or shows it again.
    pub fn toggle_hidden(&mut self, object: &SceneObject, at: usize) {
        let mut hidden = self.hidden(object).to_vec();
        match hidden.iter().position(|&shell| shell == at) {
            Some(index) => {
                hidden.remove(index);
            }
            None => hidden.push(at),
        }
        self.stand_in.clear();
        self.hidden = (!hidden.is_empty()).then(|| (Taken::of(object), hidden));
        let going: Vec<u32> = self
            .hidden(object)
            .iter()
            .filter_map(|&at| self.rows(object)?.get(at))
            .flat_map(|row| row.faces.iter().copied())
            .collect();
        if !going.is_empty() {
            let mesh = Arc::clone(&object.mesh);
            start(&mut self.stand_in, object, move || {
                Arc::new(without(&mesh, &going))
            });
        }
        // A hidden shell is not marked either, or its mark would still stand where it was.
        self.set(object, self.picked(object).to_vec());
    }

    /// The one model drawn without its hidden shells, and what it is drawn as, once made;
    /// only while it is the one model the list is of.
    pub fn stand_in(&self, scene: &Scene) -> Option<(ObjectId, Arc<Mesh>)> {
        let (taken, _) = self.hidden.as_ref()?;
        let mut targets = scene.targets();
        let (Some(object), None) = (targets.next(), targets.next()) else {
            return None;
        };
        let mesh = done(&self.stand_in, object)
            .filter(|_| object.id == taken.id)
            .flatten()?;
        Some((object.id, Arc::clone(mesh)))
    }

    /// Lets every pick go, as a drop of them has been asked for.
    pub fn clear(&mut self) {
        self.picked = None;
    }

    /// Where the camera is to go, once on the frame a shell was picked.
    pub fn take_flight(&mut self) -> Option<Spot> {
        self.flight.take()
    }

    /// Takes in the lists that are done, and forgets those of a mesh replaced since.
    /// Returns whether one is still being made.
    pub fn poll(&mut self, scene: &Scene) -> bool {
        settle(&mut self.listed, scene);
        settle(&mut self.stand_in, scene);
        let stale = self.hidden.as_ref().is_some_and(|(taken, _)| {
            !scene
                .get(taken.id)
                .is_some_and(|object| taken.is_model(object))
        });
        if stale {
            self.hidden = None;
        }
        running(&self.listed) || running(&self.stand_in)
    }

    /// The picked shells of the model they were picked on, marked in `fault`.
    pub fn highlight(&self, scene: &Scene) -> Highlight {
        let mut highlight = Highlight::default();
        let Some((taken, _, Some(patch))) = &self.picked else {
            return highlight;
        };
        let Some(object) = scene.get(taken.id).filter(|object| taken.is_model(object)) else {
            return highlight;
        };
        highlight.faces.push(FaceMark {
            mesh: Arc::clone(patch),
            transform: object.transform,
            colour: theme::scene().fault,
        });
        highlight
    }

    fn matching(&self, object: &SceneObject, keep: impl Fn(&ShellRow) -> bool) -> Vec<usize> {
        let rows = self.rows(object).unwrap_or_default();
        (0..rows.len()).filter(|&at| keep(&rows[at])).collect()
    }

    /// Puts `picked` down as the picks of `object`, with the one patch they are marked by.
    fn set(&mut self, object: &SceneObject, picked: Vec<usize>) {
        let rows = self.rows(object).unwrap_or_default();
        let hidden = self.hidden(object);
        let faces = picked
            .iter()
            .filter(|at| !hidden.contains(at))
            .filter_map(|&at| rows.get(at))
            .filter(|row| row.faces.len() <= MARKED_FACES)
            .flat_map(|row| row.faces.iter().map(|&face| face as usize));
        let marked = patch(&object.mesh, faces, LIFT_MM);
        self.picked = Some((Taken::of(object), picked, marked));
    }
}

/// `mesh` with the faces `going` left out, its vertices as they are.
fn without(mesh: &Mesh, going: &[u32]) -> Mesh {
    let mut kept = vec![true; mesh.faces.len()];
    for &face in going {
        if let Some(kept) = kept.get_mut(face as usize) {
            *kept = false;
        }
    }
    let faces = mesh
        .faces
        .iter()
        .zip(&kept)
        .filter_map(|(face, kept)| kept.then_some(*face))
        .collect();
    Mesh::new(mesh.vertices.clone(), faces)
}

/// The shells of `mesh` as the list shows them, each with where it is.
fn rows(mesh: &Mesh) -> Vec<ShellRow> {
    list_shells(mesh)
        .into_iter()
        .map(|shell| {
            let corners = shell
                .faces
                .iter()
                .flat_map(|&face| mesh.faces[face as usize]);
            // A shell faces every way, so it is framed from wherever the camera stands.
            let spot = Spot::around(
                corners.map(|corner| mesh.vertices[corner as usize]),
                Vec3::ZERO,
            );
            ShellRow {
                faces: shell.faces,
                area_mm2: shell.area_mm2,
                closed: shell.closed,
                spot,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
