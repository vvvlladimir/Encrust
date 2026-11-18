//! Several models made one by a boolean (`docs/design/document.md`).

use core_edit::{ObjectId, Op};
use core_geometry::{Mesh, Vec3, transform_mesh};
use core_volume::{
    Boolean, Cancel, Combined, CsgError, VolumeError, combine, combine_exact_toward,
};

use crate::apply::{Applied, Report};
use crate::document::Document;
use crate::error::ApplyError;
use crate::piece::{Piece, Role};

/// The volume operation `op` names.
pub fn boolean_of(op: Op) -> Boolean {
    match op {
        Op::Union => Boolean::Union,
        Op::Difference => Boolean::Difference,
        Op::Intersection => Boolean::Intersection,
    }
}

/// `placed`, models where they stand on the plate, combined by `op`, the first being what a
/// difference keeps and named `first` in an error. With `toward` set the cut is exact.
pub fn combined(
    placed: &[Mesh],
    op: Boolean,
    toward: Option<Vec3>,
    cancel: Cancel<'_>,
    first: ObjectId,
) -> Result<Combined, ApplyError> {
    if placed.len() < 2 {
        return Err(ApplyError::TooFew);
    }
    let made = match toward {
        Some(toward) => combine_exact_toward(placed, op, toward).map(|mesh| Combined {
            mesh,
            voxel_mm: None,
        }),
        None => combine(placed, op, cancel),
    };
    made.map_err(|error| match error {
        VolumeError::NothingLeft => ApplyError::NothingLeft,
        VolumeError::Csg(CsgError::NotSolid(_)) => ApplyError::NotSolid(first),
        VolumeError::Cancelled => ApplyError::Cancelled,
        other => ApplyError::Failed {
            object: first,
            reason: other.to_string(),
        },
    })
}

/// Replaces `ids` by the one object `op` makes of them, at the first's place.
pub(crate) fn apply_combine(
    document: &mut Document,
    ids: &[ObjectId],
    op: Op,
    toward: Option<Vec3>,
    cancel: Cancel<'_>,
) -> Result<Applied, ApplyError> {
    let mut ids = ids.to_vec();
    let mut seen = Vec::new();
    ids.retain(|id| {
        let new = !seen.contains(id);
        seen.push(*id);
        new
    });
    let (first, rest) = match ids.split_first() {
        Some((first, rest)) if !rest.is_empty() => (first, rest),
        _ => return Err(ApplyError::TooFew),
    };
    let mut placed = Vec::with_capacity(ids.len());
    for id in &ids {
        let (state, meshes) = document.object(*id).ok_or(ApplyError::NoObject(*id))?;
        placed.push(transform_mesh(&meshes.source, state.transform));
    }
    if cancel.asked() {
        return Err(ApplyError::Cancelled);
    }
    let made = combined(&placed, boolean_of(op), toward, cancel, *first)?;
    let added = document.replace(*first, vec![Piece::sound(Role::Combined, made.mesh)]);
    let mut removed = vec![*first];
    for id in rest {
        removed.extend(document.remove(*id));
    }
    Ok(Applied {
        removed,
        added,
        report: Report::Combined {
            voxel_mm: made.voxel_mm,
        },
    })
}

#[cfg(test)]
mod tests;
