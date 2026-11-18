use core_edit::{Edit, ObjectId};
use core_engine::project::hollow_of;
use core_geometry::{Mesh, Transform};
use core_volume::Cancel;

use crate::cut::{CutAsked, CutModel, cut, pieces};
use crate::document::Document;
use crate::error::ApplyError;
use crate::piece::Role;
use crate::placement::{moved, sized_to, turned_to};
use crate::plate_contents::{delete, duplicate, mirrored};
use crate::split::{apply_part_off, apply_split};

/// What an edit did, for a front end to name, select and report it.
#[derive(Debug, Clone, PartialEq)]
pub struct Applied {
    pub removed: Vec<ObjectId>,
    /// Each object the edit made, with what it is to the one it came from.
    pub added: Vec<(ObjectId, Role)>,
    pub report: Report,
}

/// What an edit has to say beyond the objects it changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Report {
    Cut {
        /// Cut loops that did not close, and were left uncapped.
        open_loops: usize,
        /// Pins, dowels or magnets put in, or pieces of the face a key runs round.
        placed: usize,
        /// Pieces of the cut face too small or thin to take a fixing.
        bare: usize,
        /// Why the halves were left plain when they were to be joined.
        unjoined: Option<String>,
        /// Pieces that did not come out sound and were mended.
        mended: usize,
    },
    /// An object was broken into its parts, or left whole where it was one.
    Split {
        parts: usize,
        /// Scraps too small to be parts, joined to the part they touch.
        joined: usize,
        /// Scraps too small to be parts and touching none, left out.
        dropped: usize,
    },
    /// A part was cut off an object.
    PartedOff,
    /// An object was moved, turned or sized where it stands.
    Placed,
    /// An object was deleted or copied.
    Contents,
}

/// Carries `edit` out on `document`, which an error leaves as it was; `cancel` stops one that
/// takes long enough to be given up on.
pub fn apply(
    document: &mut Document,
    edit: &Edit,
    cancel: Cancel<'_>,
) -> Result<Applied, ApplyError> {
    match *edit {
        Edit::Cut {
            object,
            axis,
            at_mm,
            keep,
            joints,
            lay_flat,
        } => {
            let asked = CutAsked {
                axis,
                at_mm,
                keep,
                joints,
                lay_flat,
            };
            apply_cut(document, object, &asked)
        }
        Edit::Split {
            object,
            smallest_mm3,
        } => apply_split(document, object, smallest_mm3, cancel),
        Edit::PartOff { object, at_mm } => apply_part_off(document, object, at_mm, cancel),
        Edit::Move { object, by_mm } => {
            place(document, object, |transform, _| Ok(moved(transform, by_mm)))
        }
        Edit::Rotate { object, degrees } => place(document, object, |transform, mesh| {
            Ok(turned_to(transform, mesh, degrees))
        }),
        Edit::Scale { object, factors } => place(document, object, |transform, mesh| {
            sized_to(transform, mesh, factors).ok_or(ApplyError::BadScale(object))
        }),
        Edit::Delete { object } => delete(document, object),
        Edit::Duplicate { object, offset_mm } => duplicate(document, object, offset_mm),
        Edit::Mirror { object, axis } => place(document, object, |transform, mesh| {
            Ok(mirrored(transform, mesh, axis))
        }),
    }
}

/// Sets the transform of `id` to what `to` makes of it and the object's own mesh.
fn place(
    document: &mut Document,
    id: ObjectId,
    to: impl FnOnce(Transform, &Mesh) -> Result<Transform, ApplyError>,
) -> Result<Applied, ApplyError> {
    let (state, meshes) = document.object(id).ok_or(ApplyError::NoObject(id))?;
    let transform = to(state.transform, &meshes.source)?;
    document.set_transform(id, transform);
    Ok(Applied {
        removed: Vec::new(),
        added: Vec::new(),
        report: Report::Placed,
    })
}

fn apply_cut(
    document: &mut Document,
    id: ObjectId,
    asked: &CutAsked,
) -> Result<Applied, ApplyError> {
    let (state, meshes) = document.object(id).ok_or(ApplyError::NoObject(id))?;
    let hollow = hollow_of(&state.hollow, meshes);
    let model = CutModel {
        printed: meshes.printed().clone(),
        cuts: hollow.cut_bodies().cloned(),
        solid: meshes.shell.as_ref().map(|_| meshes.source.clone()),
        transform: state.transform,
    };
    let done = cut(&model, asked, id)?;
    let (placed, bare) = (done.joined.placed, done.joined.bare);
    let pieces = pieces(done.joined, asked);
    let mended = pieces.iter().filter(|piece| piece.mended).count();
    let added = document.replace(id, pieces);
    Ok(Applied {
        removed: vec![id],
        added,
        report: Report::Cut {
            open_loops: done.open_loops,
            placed,
            bare,
            unjoined: done.unjoined.map(|error| error.to_string()),
            mended,
        },
    })
}

#[cfg(test)]
mod tests;
