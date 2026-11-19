//! Plugs laid beside an object for its drain holes (ADR 0313, 0321).

use std::sync::Arc;

use core_edit::ObjectId;
use core_engine::project::hollow_of;
use core_geometry::{Bvh, Mesh, Scalar, transform_mesh};
use core_volume::{
    Cancel, DrainHole, ModelRead, Plugs, VolumeError, laid_beside, plug_wall_mm, plugs,
};

use crate::apply::{Applied, Report};
use crate::document::Document;
use crate::error::ApplyError;
use crate::piece::{Piece, Role};

/// A plug for each of `holes`, as they are cut, out of the solid `mesh`: `wall_mm` long where
/// there is a wall, `gap_mm` narrower a side.
pub fn plugs_cut(
    mesh: &Mesh,
    holes: &[DrainHole],
    (wall_mm, gap_mm): (Option<Scalar>, Scalar),
    cancel: Cancel<'_>,
) -> Result<Plugs, VolumeError> {
    let read = ModelRead::new(Arc::new(mesh.clone()));
    plugs(
        &mut None,
        &read,
        &Bvh::build(mesh),
        holes,
        wall_mm,
        gap_mm,
        cancel,
    )
}

/// Lays a plug for each hole of object `id` in a row past its right-hand side, `gap_mm`
/// narrower a side than the hole, in place of the plugs laid before.
pub(crate) fn apply_plugs(
    document: &mut Document,
    id: ObjectId,
    gap_mm: Scalar,
    cancel: Cancel<'_>,
) -> Result<Applied, ApplyError> {
    let (state, meshes) = document.object(id).ok_or(ApplyError::NoObject(id))?;
    let transform = state.transform;
    let hollow = hollow_of(&state.hollow, meshes);
    let least = transform.least_scale();
    let wall_mm =
        (hollow.asked()).and_then(|asked| plug_wall_mm(asked.mode, asked.thickness_mm / least));
    let holes = hollow.cut().0;
    let made =
        plugs_cut(&meshes.source, holes, (wall_mm, gap_mm / least), cancel).map_err(|error| {
            match error {
                VolumeError::Cancelled => ApplyError::Cancelled,
                other => ApplyError::Failed {
                    object: id,
                    reason: other.to_string(),
                },
            }
        })?;
    let beside =
        (transform_mesh(meshes.printed(), transform).aabb()).ok_or(ApplyError::NoSurface(id))?;
    // `laid_beside` leaves out an empty plug, so each is numbered after its own hole.
    let numbers = (1..)
        .zip(&made.meshes)
        .filter(|(_, plug)| !plug.is_empty())
        .map(|(number, _)| number);
    let laid = laid_beside(&made.meshes, holes, transform, beside);
    let pieces: Vec<Piece> = numbers
        .zip(laid)
        .map(|(number, mesh)| Piece::sound(Role::Plug(number), mesh))
        .collect();
    let plugs = pieces.len();
    let (removed, added) = document
        .lay_plugs(id, pieces)
        .ok_or(ApplyError::NoObject(id))?;
    Ok(Applied {
        removed,
        added,
        report: Report::Plugged {
            plugs,
            closed: made.closed,
        },
    })
}
