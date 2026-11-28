//! A model hollowed by the numbers it keeps as its own (`docs/design/document.md`).

use std::sync::Arc;

use core_edit::ObjectId;
use core_engine::project::{BuiltCavity, HollowState, ModelMeshes, ObjectState, hollow_of};
use core_geometry::{Bvh, Mat3, Mesh, Scalar, Transform, Vec3};
use core_volume::{
    BLOCKER_REACH_MM, Blocker, BlockerSize, Cancel, HollowMode, HollowSettings, Hollowed,
    ModelHollow, Shell, blocker_under, carve, cavity_field, plate_down, rod_on,
};

use crate::apply::{Applied, Report};
use crate::document::Document;
use crate::error::ApplyError;
use crate::surface::surface_near;

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

/// The blockers each of `runs`, plate points, lays on a model standing under `transform`, in
/// its own space, as a click and shift-clicks lay them, `reach_mm` past the wall.
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
            let (point, normal) =
                surface_near((mesh, bvh), (transform, into_model), *near, object)?;
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
    let failed = |error| ApplyError::of_volume(object, error);
    let field = cavity_field(mesh, bvh, settings, shape, cancel).map_err(failed)?;
    let hollowed = carve(mesh, &field, settings, cancel).map_err(failed)?;
    shell_from(hollowed, shape, settings.clone()).map_err(|NoRoom| ApplyError::NoRoom(object))
}

/// How far apart two directions to the plate, unit vectors, may lie and be the same floor:
/// what a turn about the vertical leaves of float dust.
const SAME_FLOOR: Scalar = 1e-4;

/// Object `id`'s shell built again to stand at `transform` where the one standing is stale
/// there, resized or open toward another floor; `None` where it fits or the object is solid.
pub(crate) fn reshelled(
    (state, meshes): (&ObjectState, &ModelMeshes),
    transform: Transform,
    cancel: Cancel<'_>,
    id: ObjectId,
) -> Result<Option<Shell>, ApplyError> {
    let hollow = hollow_of(&state.hollow, meshes);
    let Some(built) = hollow.asked() else {
        return Ok(None);
    };
    let down = plate_down(transform);
    let toward = |was: Vec3| {
        if was.abs_diff_eq(down, SAME_FLOOR) {
            was
        } else {
            down
        }
    };
    let wanted = HollowSettings {
        open_toward: built.open_toward.map(toward),
        ..built.clone()
    };
    if !hollow.is_stale(&wanted, transform) {
        return Ok(None);
    }
    let bvh = Bvh::build(&meshes.source);
    let settings = hollow.asking(&wanted);
    let shape = transform.unturned();
    shelled((&meshes.source, &bvh), &settings, shape, cancel, id).map(Some)
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

/// An object's hollow carried onto a new surface: its blockers, its cuts and, where it was
/// hollowed, its shell built again.
pub(crate) struct Rehollowed {
    carried: ModelHollow,
    shell: Option<Shell>,
}

/// Carries `hollow`, of a surface replaced by `source` sized by `sizing`, onto `source` standing
/// at `transform` and hollows it again as built, as the window's Relief tool does.
pub(crate) fn rehollowed(
    source: &Mesh,
    (hollow, sizing): (&ModelHollow, Mat3),
    transform: Transform,
    cancel: Cancel<'_>,
    id: ObjectId,
) -> Result<Rehollowed, ApplyError> {
    let bvh = Bvh::build(source);
    let carried = hollow.carried_onto(source, &bvh, sizing);
    let shell = match hollow.asked() {
        Some(asked) => {
            let settings = carried.asking(asked);
            Some(shelled(
                (source, &bvh),
                &settings,
                transform.unturned(),
                cancel,
                id,
            )?)
        }
        None => None,
    };
    Ok(Rehollowed { carried, shell })
}

impl Rehollowed {
    /// Gives object `id` this hollow; its supports stay.
    pub(crate) fn lay_on(self, document: &mut Document, id: ObjectId) {
        let carried = self.carried;
        document.set_blockers(id, carried.blockers().to_vec());
        document.set_cuts(id, carried.drains().to_vec(), carried.channels().to_vec());
        if let Some(shell) = self.shell {
            document.set_built(id, BuiltCavity::of(&shell), shell.mesh);
        }
    }
}

#[cfg(test)]
mod tests;
