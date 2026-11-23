//! A model hollowed by the numbers it keeps as its own (`docs/design/document.md`).

use std::sync::Arc;

use core_edit::ObjectId;
use core_engine::project::{BuiltCavity, HollowState, hollow_of};
use core_geometry::{Bvh, Mat3, Mesh, Scalar, Transform, Vec3};
use core_volume::{
    BLOCKER_REACH_MM, Blocker, BlockerSize, Cancel, HollowMode, HollowSettings, Hollowed, Shell,
    VolumeError, blocker_under, carve, cavity_field, plate_down, rod_on,
};

use crate::apply::{Applied, Report};
use crate::document::Document;
use crate::error::ApplyError;

/// The run settings a panel's numbers ask for, on a model standing under `transform`. An
/// open floor faces the plate it stands on, so turning such a model makes its shell stale.
pub fn settings_of(state: &HollowState, transform: Transform) -> HollowSettings {
    let opens = state.open_floor && state.mode == HollowMode::Internal;
    HollowSettings {
        thickness_mm: state.thickness_mm,
        mode: state.mode,
        precision: state.precision,
        infill: state.infill_on.then_some(state.infill),
        open_toward: opens.then(|| plate_down(transform)),
        solid_under_mm: state.solid_under_mm,
        ..HollowSettings::default()
    }
}

/// The blockers each of `runs` lays on a model standing under `transform`, in its own space: a
/// ball at the surface nearest each run's first plate point and a rod on to each next, as a
/// click and shift-clicks lay them; reaching `reach_mm` past the wall, fitted where unset.
pub fn blockers_along(
    (mesh, bvh): (&Mesh, &Bvh),
    transform: Transform,
    (runs, reach_mm): (&[Vec<Vec3>], Option<Scalar>),
    object: ObjectId,
) -> Result<Vec<Blocker>, ApplyError> {
    let size = BlockerSize {
        fit: reach_mm.is_none(),
        reach_mm: reach_mm.unwrap_or(BLOCKER_REACH_MM),
    };
    let into_model = transform
        .inverse_matrix()
        .ok_or(ApplyError::BadTransform(object))?;
    let mut placed = Vec::new();
    for run in runs {
        let mut last: Option<Blocker> = None;
        for near in run {
            let closest = (bvh.closest(into_model.transform_point3(*near)))
                .ok_or(ApplyError::NoSurface(object))?;
            let face = mesh
                .triangle(closest.face)
                .ok_or(ApplyError::NoSurface(object))?;
            let normal =
                (transform.normal_matrix() * face.normal_unnormalized()).normalize_or_zero();
            let point = transform.to_matrix().transform_point3(closest.point);
            let ball = blocker_under(mesh, bvh, transform, point, normal, size)
                .ok_or(ApplyError::NoSurface(object))?;
            let next = last.map_or(ball, |last| rod_on(&last, &ball));
            placed.push(next);
            last = Some(next);
        }
    }
    Ok(placed)
}

/// A wall that leaves no cavity to hollow a model by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoRoom;

/// What a carve left, as the shell of a model measured under `shape` and built to `settings`;
/// `NoRoom` where an inside wall left no cavity.
pub fn shell_from(
    hollowed: Hollowed,
    shape: Mat3,
    settings: HollowSettings,
) -> Result<Shell, NoRoom> {
    if hollowed.cavity.is_empty() && settings.mode == HollowMode::Internal {
        return Err(NoRoom);
    }
    Ok(Shell {
        mesh: Arc::new(hollowed.mesh),
        cavity: hollowed.cavity,
        cavity_mm3: hollowed.cavity_mm3,
        voxel_mm: hollowed.voxel_mm,
        coarsened: hollowed.coarsened,
        shape,
        settings,
    })
}

/// `mesh` hollowed as `settings` ask, measured on the plate under `shape`
/// (`Transform::unturned`); `object` names the model in an error.
pub fn shelled(
    (mesh, bvh): (&Mesh, &Bvh),
    settings: &HollowSettings,
    shape: Mat3,
    cancel: Cancel<'_>,
    object: ObjectId,
) -> Result<Shell, ApplyError> {
    let failed = |error: VolumeError| match error {
        VolumeError::Cancelled => ApplyError::Cancelled,
        other => ApplyError::Failed {
            object,
            reason: other.to_string(),
        },
    };
    let field = cavity_field(mesh, bvh, settings, shape, cancel).map_err(failed)?;
    let hollowed = carve(mesh, &field, settings, cancel).map_err(failed)?;
    shell_from(hollowed, shape, settings.clone()).map_err(|NoRoom| ApplyError::NoRoom(object))
}

/// Hollows the object `id` by `settings`, which it keeps as its own.
pub(crate) fn apply_hollow(
    document: &mut Document,
    id: ObjectId,
    (settings, (runs, reach_mm)): (&HollowState, (&[Vec<Vec3>], Option<Scalar>)),
    cancel: Cancel<'_>,
) -> Result<Applied, ApplyError> {
    let (state, meshes) = document.object(id).ok_or(ApplyError::NoObject(id))?;
    let transform = state.transform;
    let bvh = Bvh::build(&meshes.source);
    let mut hollow = hollow_of(&state.hollow, meshes);
    for blocker in blockers_along((&meshes.source, &bvh), transform, (runs, reach_mm), id)? {
        hollow.place_blocker(blocker);
    }
    let blockers = hollow.blockers().to_vec();
    let asked = hollow.asking(&settings_of(settings, transform));
    let shell = shelled(
        (&meshes.source, &bvh),
        &asked,
        transform.unturned(),
        cancel,
        id,
    )?;
    let report = Report::Hollowed {
        cavity_mm3: shell.cavity_mm3,
        voxel_mm: shell.voxel_mm,
        coarsened: shell.coarsened,
    };
    document.set_shell(id, BuiltCavity::of(&shell), shell.mesh, settings.clone());
    document.set_blockers(id, blockers);
    Ok(Applied {
        removed: Vec::new(),
        added: Vec::new(),
        report,
    })
}

#[cfg(test)]
mod tests;
