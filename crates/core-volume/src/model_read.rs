use std::ops::Range;
use std::sync::{Arc, OnceLock};

use core_csg::{CsgError, Solid};
use core_geometry::{FastMap, Mesh};

use crate::cancel::{Cancel, Progress, Step};
use crate::error::VolumeError;
use crate::mend::readable;

/// One model read into the engine once, for whichever of its drill and its plugs asks first;
/// the other waits for that read rather than making its own (ADR 0412).
#[derive(Debug)]
pub struct ModelRead {
    model: Arc<Mesh>,
    solid: OnceLock<Result<Solid, CsgError>>,
    /// How far the read has got, for every run waiting on it to show.
    progress: Progress,
}

impl ModelRead {
    /// `model`, not read yet.
    pub fn new(model: Arc<Mesh>) -> Self {
        Self {
            model,
            solid: OnceLock::new(),
            progress: Progress::default(),
        }
    }

    /// The model that begins `shell`: its faces before `cavity`, on the vertices they reach.
    pub fn of_shell(shell: &Mesh, cavity: &Range<usize>) -> Self {
        let faces = shell.faces[..cavity.start.min(shell.faces.len())].to_vec();
        let reached = faces
            .iter()
            .flatten()
            .max()
            .map_or(0, |at| *at as usize + 1);
        let vertices = shell.vertices[..reached.min(shell.vertices.len())].to_vec();
        Self::new(Arc::new(Mesh::new(vertices, faces)))
    }

    /// The model this reads.
    pub fn model(&self) -> &Arc<Mesh> {
        &self.model
    }

    /// How far the read has got, as whichever run made it was told.
    pub fn progress(&self) -> &Progress {
        &self.progress
    }

    /// The model as the engine reads it, mended where it must be: read on the first ask while
    /// a second waits for it, and not started for a run already given up.
    pub fn solid(&self, cancel: Cancel<'_>) -> Result<&Solid, VolumeError> {
        if cancel.asked() {
            return Err(VolumeError::Cancelled);
        }
        cancel.begin(Step::Reading, 0);
        self.solid
            .get_or_init(|| readable(&self.model, Cancel::never().reporting(&self.progress)))
            .as_ref()
            .map_err(|refused| refused.clone().into())
    }

    /// `shell` built on this read: the model less its cavity, with its infill. `None` where the
    /// model does not begin it byte for byte (ADR 0412), a part is refused or the run given up.
    pub fn shell(&self, shell: &Mesh, cavity: &Range<usize>, cancel: Cancel<'_>) -> Option<Solid> {
        let model = &*self.model;
        let begins_with_model = cavity.start == model.faces.len()
            && shell.faces.get(..cavity.start) == Some(&model.faces[..])
            && shell.vertices.get(..model.vertices.len()) == Some(&model.vertices[..]);
        if !begins_with_model || cavity.end > shell.faces.len() {
            return None;
        }
        let going = || (!cancel.asked()).then_some(());
        let whole = self.solid(cancel).ok()?;
        cancel.begin(Step::Combining, 0);
        going()?;
        let void = readable(&faces_of(shell, cavity.clone(), true), Cancel::never()).ok()?;
        going()?;
        let walled = whole.difference(&void).ok()?;
        if cavity.end == shell.faces.len() {
            return Some(walled);
        }
        going()?;
        let infill = faces_of(shell, cavity.end..shell.faces.len(), false);
        let infill = readable(&infill, Cancel::never()).ok()?;
        Solid::union_all(&[walled, infill]).ok()
    }
}

/// The faces `range` of `mesh` as a mesh of their own, turned over when `flip`.
fn faces_of(mesh: &Mesh, range: Range<usize>, flip: bool) -> Mesh {
    let mut index: FastMap<u32, u32> = FastMap::default();
    let mut vertices = Vec::new();
    let mut corner = |at: u32| {
        *index.entry(at).or_insert_with(|| {
            vertices.push(mesh.vertices[at as usize]);
            (vertices.len() - 1) as u32
        })
    };
    let faces = mesh.faces[range]
        .iter()
        .map(|&[a, b, c]| {
            let (a, b, c) = (corner(a), corner(b), corner(c));
            if flip { [a, c, b] } else { [a, b, c] }
        })
        .collect();
    Mesh::new(vertices, faces)
}

#[cfg(test)]
mod tests;
