//! Holes drilled where the user points, kept in the object's own space (`docs/design/document.md`).

use core_edit::Edit;
use core_edit::ObjectId;
use core_engine::project::hollow_of;
use core_geometry::Transform;
use core_geometry::{Bvh, Mesh, Scalar, Vec3};
use core_volume::{Cancel, Channel, DrainHole, HoleKind, HoleSize, drill, hole_at};

use crate::apply::{Applied, Report};
use crate::document::Document;
use crate::error::ApplyError;

mod auto;
mod plugs;

pub use auto::{SearchAsked, drains_proven, plate_under};
use plugs::apply_plugs;
pub use plugs::plugs_cut;

/// Past this, the point is off an edge or corner rather than over the face the hole is
/// drilled into: a cube's edge is 45 degrees off, a 12-sided pillar's facets 15.
const OFF_A_FACE_DEG: Scalar = 20.0;

/// Nearer than this a point is on the surface: the f32 closest point lands a few ulps off it
/// in any direction, so the offset says nothing of which way the point lies.
const ON_THE_SURFACE_MM: Scalar = 1e-3;

/// A hole of `size` on the surface nearest `near`, which stands `up` over the model as it is
/// here, drilled straight into it, refused off an edge or a corner. `object` names the model
/// in an error.
pub fn hole_near(
    mesh: &Mesh,
    bvh: &Bvh,
    (near, up): (Vec3, Vec3),
    (size, kind): (HoleSize, HoleKind),
    object: ObjectId,
) -> Result<DrainHole, ApplyError> {
    let hole = hole_at(mesh, bvh, near - up, size).ok_or(ApplyError::NoSurface(object))?;
    if off_a_face(near, &hole, up) {
        return Err(ApplyError::OffAFace {
            object,
            at_mm: near,
            mouth_mm: hole.at + up,
        });
    }
    Ok(DrainHole { kind, ..hole })
}

/// Whether `hole` sits half off the model, put there by a point off an edge or a corner, where
/// a click always lands on a face. A point over or under a face is off it along its normal,
/// either way. `near` stands `up` over the model as it is here.
fn off_a_face(near: Vec3, hole: &DrainHole, up: Vec3) -> bool {
    let off = near - up - hole.at;
    let Some(inward) = hole
        .axis
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
        Edit::Plugs { object, gap_mm } => apply_plugs(document, object, gap_mm, cancel),
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

/// Drills a hole of `size` and `kind` into the surface of object `id` nearest each of `at_mm`,
/// then the holes the engine's search proves drain its cavity where `searching.auto`, which the
/// object keeps with its cavity numbers; the shell is not rebuilt.
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
    let inverse = state
        .transform
        .inverse_matrix()
        .ok_or(ApplyError::BadTransform(id))?;
    let bvh = Bvh::build(&meshes.source);
    let mut hollow = hollow_of(&state.hollow, meshes);
    let mut holes = 0;
    for plate in at_mm {
        let own = inverse.transform_point3(*plate);
        let hole = hole_near(&meshes.source, &bvh, (own, Vec3::ZERO), (size, kind), id).map_err(
            |error| match error {
                ApplyError::OffAFace {
                    object, mouth_mm, ..
                } => ApplyError::OffAFace {
                    object,
                    at_mm: *plate,
                    mouth_mm: state.transform.to_matrix().transform_point3(mouth_mm),
                },
                other => other,
            },
        )?;
        holes += usize::from(hollow.add_hole(hole));
    }
    let mut trapped = 0;
    if searching.auto {
        let on = (meshes.printed().as_ref(), &hollow, state.transform);
        let found = auto::search_object(on, (size, searching.vent), (id, cancel))?;
        trapped = found.trapped.len();
        for hole in found.drain_holes() {
            let own = hole.into_model(state.transform);
            holes += usize::from(own.is_some_and(|hole| hollow.add_hole(hole)));
        }
    }
    let (drains, channels) = (hollow.drains().to_vec(), hollow.channels().to_vec());
    document.set_cuts(id, drains, channels);
    Ok(Applied {
        removed: Vec::new(),
        added: Vec::new(),
        report: Report::Drained { holes, trapped },
    })
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
