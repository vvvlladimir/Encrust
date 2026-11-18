use core_edit::{Edit, ObjectId};
use core_engine::project::hollow_of;

use crate::cut::{CutAsked, CutModel, cut, pieces};
use crate::document::Document;
use crate::error::ApplyError;
use crate::piece::Role;

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
}

/// Carries `edit` out on `document`, which an error leaves as it was.
pub fn apply(document: &mut Document, edit: &Edit) -> Result<Applied, ApplyError> {
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
    }
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
