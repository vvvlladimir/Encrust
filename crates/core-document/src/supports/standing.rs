//! The supports an object keeps, restored where it stands, and what other edits read of them.

use std::sync::Arc;

use core_edit::ObjectId;
use core_engine::SupportPlan;
use core_engine::project::{ModelMeshes, ObjectState, supports_of};
use core_geometry::{Bvh, Mesh};
use core_supports::ModelSupports;
use printer_profiles::{Catalogue, SupportProfile};

use super::groups::{sized_plan, table_of};
use crate::document::Document;
use crate::error::ApplyError;
use crate::orient::orient_plan;

/// The supports `state` keeps, restored on the printed surface of `meshes` where it stands.
pub(crate) fn kept_supports(
    state: &ObjectState,
    meshes: &ModelMeshes,
    bvh: &Bvh,
    table: &[SupportProfile],
) -> ModelSupports {
    let (source, drains) = (&meshes.source, &state.hollow.drains);
    supports_of(
        &state.supports,
        drains,
        source,
        meshes.printed(),
        bvh,
        state.transform,
        table,
    )
}

/// The raft under object `id` where it stands, plate mm; none where it stands on no supports.
pub(crate) fn raft_under(document: &Document, id: ObjectId) -> Option<Arc<Mesh>> {
    let (state, meshes) = document.object(id)?;
    let plate = document.support_state();
    let table = table_of(state, &plate.groups, plate.raft);
    let bvh = Bvh::build(meshes.printed());
    kept_supports(state, meshes, &bvh, &table).raft().cloned()
}

/// The meshes of the supports under object `id` where it stands, plate mm, columns and raft.
pub fn standing_meshes(document: &Document, id: ObjectId) -> Vec<Arc<Mesh>> {
    let Some((state, meshes)) = document.object(id) else {
        return Vec::new();
    };
    if state.supports.points.is_empty() {
        return Vec::new();
    }
    let plate = document.support_state();
    let table = table_of(state, &plate.groups, plate.raft);
    let bvh = Bvh::build(meshes.printed());
    kept_supports(state, meshes, &bvh, &table)
        .every_mesh()
        .cloned()
        .collect()
}

/// The plan an orientation of `state` weighs: the plate's kept sizing, sized, where it is on,
/// else the active group's profile; none where the plate has no group.
pub(crate) fn orient_plan_of(
    document: &Document,
    state: &ObjectState,
) -> Result<Option<SupportPlan>, ApplyError> {
    let plate = document.support_state();
    let table = table_of(state, &plate.groups, plate.raft);
    let Some(active) = table.get(usize::from(plate.active)).or(table.first()) else {
        return Ok(None);
    };
    let kept = plate.sizing.as_ref().filter(|sizing| sizing.on);
    let sized = kept
        .map(|sizing| sized_plan(Catalogue::bundled(), (Some(sizing), &plate.groups), active))
        .transpose()?;
    Ok(Some(orient_plan(sized, active, active.raft)))
}
