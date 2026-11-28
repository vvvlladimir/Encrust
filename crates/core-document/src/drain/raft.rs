//! Holes drilled into the raft an object stands on, kept with the object's holes as
//! `--drain-support-at` drills them.

use core_edit::ObjectId;
use core_geometry::{Bvh, Mesh, Transform, Vec3};
use core_volume::{DrainHole, HoleKind, HoleSize, hole_at};

use crate::document::Document;
use crate::error::ApplyError;
use crate::supports::raft_under;

/// The holes of `size` drilled into the raft under object `id` nearest each of `at_mm`, plate
/// mm, in the object's own space as its other holes are kept.
pub(super) fn raft_holes(
    document: &Document,
    id: ObjectId,
    at_mm: &[Vec3],
    size: HoleSize,
) -> Result<Vec<DrainHole>, ApplyError> {
    let raft = raft_under(document, id).ok_or(ApplyError::NoRaft(id))?;
    let (state, _) = document.object(id).ok_or(ApplyError::NoObject(id))?;
    holes_in_raft((&raft, state.transform), at_mm, size, id)
}

/// The holes of `size` drilled into `raft`, plate mm, nearest each of `at_mm`, in the space of
/// object `id` placed by `transform`, as a model keeps its holes.
pub fn holes_in_raft(
    (raft, transform): (&Mesh, Transform),
    at_mm: &[Vec3],
    size: HoleSize,
    id: ObjectId,
) -> Result<Vec<DrainHole>, ApplyError> {
    let bvh = Bvh::build(raft);
    (at_mm.iter())
        .map(|near| {
            let hole = hole_at(raft, &bvh, *near, size).ok_or(ApplyError::NoSurface(id))?;
            let hole = DrainHole {
                kind: HoleKind::Support,
                ..hole
            };
            hole.into_model(transform)
                .ok_or(ApplyError::BadTransform(id))
        })
        .collect()
}

#[cfg(test)]
mod tests;
