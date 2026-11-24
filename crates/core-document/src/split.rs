//! A model broken into its bodies, or one body cut off it (`docs/design/document.md`).

use core_edit::ObjectId;
use core_geometry::{Mesh, Scalar, Vec3, transform_mesh};
use core_volume::{Cancel, PartSettings, PartedOff, Parts, VolumeError, part_off, parts};

use crate::apply::{Applied, Report};
use crate::document::Document;
use crate::error::ApplyError;
use crate::piece::{Piece, Role};

/// `placed`, a model where it stands on the plate, broken into its parts.
pub fn split(placed: &Mesh, settings: &PartSettings) -> Parts {
    parts(placed, settings)
}

/// The settings a split asks for: the default smallest part where `smallest_mm3` is unset.
pub fn part_settings(smallest_mm3: Option<Scalar>) -> PartSettings {
    smallest_mm3.map_or_else(PartSettings::default, |smallest_mm3| PartSettings {
        smallest_mm3,
    })
}

/// The part of `placed` at plate point `at` cut off the rest, where `id` names the model in
/// the error.
pub fn part_off_at(
    placed: &Mesh,
    at: Vec3,
    cancel: Cancel<'_>,
    id: ObjectId,
) -> Result<PartedOff, ApplyError> {
    part_off(placed, at, cancel).map_err(|error| match error {
        VolumeError::NoNeck | VolumeError::NotOnModel => ApplyError::NoNeck(id),
        other => ApplyError::of_volume(id, other),
    })
}

/// Replaces `id` by the parts it is made of; one part and no scrap joined or dropped leaves
/// it as it was.
pub(crate) fn apply_split(
    document: &mut Document,
    id: ObjectId,
    smallest_mm3: Option<Scalar>,
    cancel: Cancel<'_>,
) -> Result<Applied, ApplyError> {
    let placed = placed_of(document, id, cancel)?;
    let found = split(&placed, &part_settings(smallest_mm3));
    let report = Report::Split {
        parts: found.parts.len(),
        joined: found.joined,
        dropped: found.dropped,
    };
    if found.parts.len() < 2 && found.joined + found.dropped == 0 {
        return Ok(Applied {
            removed: Vec::new(),
            added: Vec::new(),
            report,
        });
    }
    Ok(replaced_by(document, id, found.parts, report))
}

/// Replaces `id` by the part at `at_mm` and the rest.
pub(crate) fn apply_part_off(
    document: &mut Document,
    id: ObjectId,
    at_mm: Vec3,
    cancel: Cancel<'_>,
) -> Result<Applied, ApplyError> {
    let placed = placed_of(document, id, cancel)?;
    let parted = part_off_at(&placed, at_mm, cancel, id)?;
    Ok(replaced_by(
        document,
        id,
        vec![parted.part, parted.rest],
        Report::PartedOff,
    ))
}

/// What prints of `id`, where it stands on the plate, unless the run is already stopped.
fn placed_of(document: &Document, id: ObjectId, cancel: Cancel<'_>) -> Result<Mesh, ApplyError> {
    let (state, meshes) = document.object(id).ok_or(ApplyError::NoObject(id))?;
    if cancel.asked() {
        return Err(ApplyError::Cancelled);
    }
    Ok(transform_mesh(meshes.printed(), state.transform))
}

fn replaced_by(
    document: &mut Document,
    id: ObjectId,
    bodies: Vec<Mesh>,
    report: Report,
) -> Applied {
    let pieces = (bodies.into_iter().enumerate())
        .map(|(index, mesh)| Piece::sound(Role::Part(index + 1), mesh))
        .collect();
    let added = document.replace(id, pieces);
    Applied {
        removed: vec![id],
        added,
        report,
    }
}

#[cfg(test)]
mod tests;
