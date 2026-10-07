//! What a project's models are put back together from: the file holds every decision, so
//! opening one only meshes and cuts what the decisions already fix (ADR 0191).

use std::sync::Arc;

use core_geometry::{Bvh, Mesh, Transform};
use core_supports::ModelSupports;
use core_volume::{ModelHollow, Shell};
use printer_profiles::SupportProfile;

use super::{ModelMeshes, ObjectHollowState, ObjectSupportState};

/// The cavity an object carries, as the file left it: its shell is read, not hollowed
/// again, and the holes are cut against the wall that shell stands at.
pub fn hollow_of(state: &ObjectHollowState, meshes: &ModelMeshes) -> ModelHollow {
    let mut hollow = ModelHollow::restored(
        state.blockers.clone(),
        state.drains.clone(),
        state.channels.clone(),
    );
    let (Some(built), Some(mesh)) = (&state.built, &meshes.shell) else {
        return hollow;
    };
    let settings = hollow.asking(&built.wall.settings());
    hollow.take(Shell {
        mesh: Arc::clone(mesh),
        cavity: built.cavity_faces.clone(),
        cavity_mm3: built.cavity_mm3,
        voxel_mm: built.voxel_mm,
        coarsened: built.coarsened,
        scale: built.scale,
        settings,
    });
    hollow
}

/// The supports an object carries, standing where the file says they stand: the trees are
/// read and meshed, and no run decides where a column goes again.
pub fn supports_of(
    state: &ObjectSupportState,
    model: &Mesh,
    bvh: &Bvh,
    transform: Transform,
    table: &[SupportProfile],
) -> ModelSupports {
    let mut supports = ModelSupports::restore(
        state.points.clone(),
        state.painted.clone(),
        state.blocked.clone(),
        state.frozen.clone(),
    );
    supports.stand(&state.grown, model, bvh, transform, table);
    supports
}
