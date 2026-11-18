//! What is on the plate: an object deleted, copied or mirrored.

use core_edit::{Axis, ObjectId};
use core_geometry::{Mesh, Transform, Vec3, transform_mesh};

use crate::apply::{Applied, Report};
use crate::document::Document;
use crate::error::ApplyError;
use crate::piece::Role;
use crate::placement::moved;

/// Takes `id` and its plugs off the plate.
pub(crate) fn delete(document: &mut Document, id: ObjectId) -> Result<Applied, ApplyError> {
    let removed = document.remove(id);
    if removed.is_empty() {
        return Err(ApplyError::NoObject(id));
    }
    Ok(Applied {
        removed,
        added: Vec::new(),
        report: Report::Contents,
    })
}

/// Puts a copy of `id` `offset_mm` from it, a plug of nothing.
pub(crate) fn duplicate(
    document: &mut Document,
    id: ObjectId,
    offset_mm: Vec3,
) -> Result<Applied, ApplyError> {
    let copy = document
        .copy_after(id, |state| {
            state.transform = moved(state.transform, offset_mm);
            state.plug_of = None;
        })
        .ok_or(ApplyError::NoObject(id))?;
    Ok(Applied {
        removed: Vec::new(),
        added: vec![(copy, Role::Copy)],
        report: Report::Contents,
    })
}

/// `transform` with its scale negated along `axis` of the object's own frame, moved back so
/// the centre of the bounds of `mesh` where it stands is where it was.
pub fn mirrored(transform: Transform, mesh: &Mesh, axis: Axis) -> Transform {
    let flip = match axis {
        Axis::X => Vec3::new(-1.0, 1.0, 1.0),
        Axis::Y => Vec3::new(1.0, -1.0, 1.0),
        Axis::Z => Vec3::new(1.0, 1.0, -1.0),
    };
    let turned = Transform {
        scale: transform.scale * flip,
        ..transform
    };
    let centre = |at: Transform| {
        transform_mesh(mesh, at)
            .aabb()
            .map(|bounds| (bounds.mins + bounds.maxs) / 2.0)
    };
    match (centre(transform), centre(turned)) {
        (Some(before), Some(after)) => moved(turned, before - after),
        _ => turned,
    }
}

#[cfg(test)]
mod tests;
