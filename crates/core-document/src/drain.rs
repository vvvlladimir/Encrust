//! Holes drilled where the user points, kept in the object's own space (`docs/design/document.md`).

use core_edit::Edit;
use core_edit::ObjectId;
use core_engine::project::{hollow_of, supports_without_drilled};
use core_geometry::Transform;
use core_geometry::{Bvh, Mesh, Scalar, Vec3};
use core_volume::{Cancel, Channel, DrainHole, HoleKind, HoleSize, ModelHollow, drill};

use crate::apply::{Applied, Report};
use crate::document::Document;
use crate::error::ApplyError;
use crate::surface::surface_near;

mod auto;
mod plugs;
mod raft;

pub use auto::{SearchAsked, drains_proven, plate_under};
use plugs::apply_plugs;
pub use plugs::spaced_plugs;
pub use raft::holes_in_raft;
use raft::raft_holes;

/// Past this, the point is off an edge or corner rather than over the face the hole is
/// drilled into: a cube's edge is 45 degrees off, a 12-sided pillar's facets 15.
const OFF_A_FACE_DEG: Scalar = 20.0;

/// Nearer than this a point is on the surface: the f32 closest point lands a few ulps off it
/// in any direction, so the offset says nothing of which way the point lies.
const ON_THE_SURFACE_MM: Scalar = 1e-3;

/// The hole a click at `near` (plate mm) drills into model `object` standing under `transform`;
/// refused off an edge or corner.
pub fn hole_near(
    (mesh, bvh): (&Mesh, &Bvh),
    transform: Transform,
    (near, up): (Vec3, Vec3),
    (size, kind): (HoleSize, HoleKind),
    object: ObjectId,
) -> Result<DrainHole, ApplyError> {
    let inverse = transform
        .inverse_matrix()
        .ok_or(ApplyError::BadTransform(object))?;
    let (point, normal) = surface_near((mesh, bvh), (transform, inverse), near - up, object)?;
    let hole = ModelHollow::drain_at(mesh, bvh, point, normal, size, transform)
        .ok_or(ApplyError::NoSurface(object))?;
    if off_a_face(near, (point, -normal), up) {
        return Err(ApplyError::OffAFace {
            object,
            at_mm: near,
            mouth_mm: point + up,
        });
    }
    Ok(DrainHole { kind, ..hole })
}

/// Whether the hole at `at` along `axis`, plate mm, sits half off the model for a point off
/// an edge or corner; `near` stands `up` over the model.
fn off_a_face(near: Vec3, (at, axis): (Vec3, Vec3), up: Vec3) -> bool {
    let off = near - up - at;
    let Some(inward) = axis
        .try_normalize()
        .filter(|_| off.length() > ON_THE_SURFACE_MM)
    else {
        return false;
    };
    let off = off.normalize();
    off.dot(inward).abs().min(1.0).acos().to_degrees() > OFF_A_FACE_DEG
}

/// Carries out a `Drain`, `Channel` or `Plugs` edit.
pub(crate) fn apply_drains(
    document: &mut Document,
    edit: &Edit,
    asked_of: ObjectId,
    cancel: Cancel<'_>,
) -> Result<Applied, ApplyError> {
    match *edit {
        Edit::Drain {
            object,
            ref at_mm,
            size,
            kind,
            auto,
            vent,
        } => {
            let searching = Searching { auto, vent };
            apply_drain(document, object, at_mm, (size, kind), (searching, cancel))
        }
        Edit::Channel {
            object,
            ref points_mm,
            diameter_mm,
        } => apply_channel(document, object, points_mm, diameter_mm),
        Edit::Plugs {
            object,
            gap_mm,
            plate_mm,
            low_mm,
        } => {
            let area_mm = plate_mm.map(|plate| (low_mm, plate));
            apply_plugs(document, object, gap_mm, area_mm, cancel)
        }
        _ => Err(ApplyError::NoObject(asked_of)),
    }
}

/// What a `Drain` edit asks beyond its points: whether the engine searches too, and puts a
/// vent on each pocket it drains.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Searching {
    pub auto: bool,
    pub vent: bool,
}

/// Drills holes into object `id` at the surface nearest each of `at_mm`, then, where asked,
/// those the engine's search proves drain its cavity; the shell is not rebuilt.
pub(crate) fn apply_drain(
    document: &mut Document,
    id: ObjectId,
    at_mm: &[Vec3],
    (size, kind): (HoleSize, HoleKind),
    (searching, cancel): (Searching, Cancel<'_>),
) -> Result<Applied, ApplyError> {
    let (state, meshes) = document.object(id).ok_or(ApplyError::NoObject(id))?;
    if searching.auto && meshes.shell.is_none() {
        return Err(ApplyError::NoCavity(id));
    }
    let mut hollow = hollow_of(&state.hollow, meshes);
    let kept_holes = hollow.drains().len();
    let mut holes = drill_asked(document, id, at_mm, (size, kind), &mut hollow)?;
    let (mut trapped, mut searched) = (0, None);
    if searching.auto {
        let on = (meshes.printed().as_ref(), &hollow, state.transform);
        let found = auto::search_object(on, (size, searching.vent), (id, cancel))?;
        trapped = found.trapped.len();
        for hole in found.drain_holes() {
            let own = hole.into_model(state.transform);
            holes += usize::from(own.is_some_and(|hole| hollow.add_hole(hole)));
        }
        searched = Some(found);
    }
    let (drains, channels) = (hollow.drains().to_vec(), hollow.channels().to_vec());
    let drilled = drains.get(kept_holes..).unwrap_or_default().to_vec();
    document.set_cuts(id, drains, channels);
    clear_drilled_out(document, id, &drilled);
    Ok(Applied {
        removed: Vec::new(),
        added: Vec::new(),
        report: Report::Drained {
            holes,
            trapped,
            searched,
        },
    })
}

/// Adds to `hollow` the holes asked at `at_mm` on object `id`, into its raft for a support
/// hole; answers how many went in, one an earlier hole takes in adding none.
fn drill_asked(
    document: &Document,
    id: ObjectId,
    at_mm: &[Vec3],
    (size, kind): (HoleSize, HoleKind),
    hollow: &mut ModelHollow,
) -> Result<usize, ApplyError> {
    let mut holes = 0;
    if kind == HoleKind::Support {
        for hole in raft_holes(document, id, at_mm, size)? {
            holes += usize::from(hollow.add_hole(hole));
        }
        return Ok(holes);
    }
    let (state, meshes) = document.object(id).ok_or(ApplyError::NoObject(id))?;
    let bvh = Bvh::build(&meshes.source);
    for plate in at_mm {
        let near = (*plate, Vec3::ZERO);
        let on = (meshes.source.as_ref(), &bvh);
        let hole = hole_near(on, state.transform, near, (size, kind), id)?;
        holes += usize::from(hollow.add_hole(hole));
    }
    Ok(holes)
}

/// Takes away the supports of object `id` whose tips the `drilled` holes (its own space) take
/// out, as the window's drilling does.
fn clear_drilled_out(document: &mut Document, id: ObjectId, drilled: &[DrainHole]) {
    let Some((state, _)) = document.object(id) else {
        return;
    };
    let (supports, taken) = supports_without_drilled(&state.supports, drilled);
    if taken > 0 {
        let (points, frozen) = (supports.points().to_vec(), supports.frozen().to_vec());
        document.set_drilled_supports(id, (points, frozen));
    }
}

/// The channel of `diameter_mm` along `points_mm`, plate mm, in the own space of object `id`
/// standing under `transform`, refused where it cannot be drilled.
pub fn channel_in(
    transform: Transform,
    points_mm: &[Vec3],
    diameter_mm: Scalar,
    id: ObjectId,
) -> Result<Channel, ApplyError> {
    let inverse = transform
        .inverse_matrix()
        .ok_or(ApplyError::BadTransform(id))?;
    let channel = Channel {
        points: (points_mm.iter())
            .map(|point| inverse.transform_point3(*point))
            .collect(),
        diameter_mm: diameter_mm / transform.least_scale(),
        ..Channel::default()
    };
    drill(&[], std::slice::from_ref(&channel)).map_err(|error| ApplyError::Failed {
        object: id,
        reason: error.to_string(),
    })?;
    Ok(channel)
}

/// Digs a channel of `diameter_mm` along `points_mm` into object `id`, which it keeps with its
/// cavity numbers; the shell is not rebuilt.
pub(crate) fn apply_channel(
    document: &mut Document,
    id: ObjectId,
    points_mm: &[Vec3],
    diameter_mm: Scalar,
) -> Result<Applied, ApplyError> {
    let (state, meshes) = document.object(id).ok_or(ApplyError::NoObject(id))?;
    let channel = channel_in(state.transform, points_mm, diameter_mm, id)?;
    let mut hollow = hollow_of(&state.hollow, meshes);
    hollow.add_channel(channel);
    let (drains, channels) = (hollow.drains().to_vec(), hollow.channels().to_vec());
    document.set_cuts(id, drains, channels);
    Ok(Applied {
        removed: Vec::new(),
        added: Vec::new(),
        report: Report::Channelled,
    })
}

#[cfg(test)]
mod tests;
