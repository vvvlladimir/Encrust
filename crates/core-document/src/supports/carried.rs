use core_edit::ObjectId;
use core_engine::project::BuiltCavity;
use core_geometry::{Bvh, Transform};
use core_volume::Cancel;

use super::groups::table_of;
use super::kept_supports;
use crate::document::Document;
use crate::error::ApplyError;
use crate::hollow::reshelled;

/// Stands object `id` at `transform`, frozen trees replanted or thawed (ADR 0439), grown ones
/// dropped to grow again, and a shell stale there built again, as the window carries them.
pub(crate) fn stand_at(
    document: &mut Document,
    id: ObjectId,
    transform: Transform,
    cancel: Cancel<'_>,
) -> Result<(), ApplyError> {
    let Some((state, meshes)) = document.object(id) else {
        return Ok(());
    };
    if state.transform == transform {
        return Ok(());
    }
    let shell = reshelled((state, meshes), transform, cancel, id)?;
    let kept = if state.supports.frozen.is_empty() {
        (state.supports.points.clone(), Vec::new(), Vec::new())
    } else {
        let plate = document.support_state();
        let table = table_of(state, &plate.groups, plate.raft);
        let surface = meshes.printed();
        let bvh = Bvh::build(surface);
        let mut supports = kept_supports(state, meshes, &bvh, &table);
        supports.refresh_on(&meshes.source, surface, &bvh, transform, &table);
        let grown = supports.grown(transform);
        (
            supports.points().to_vec(),
            supports.frozen().to_vec(),
            grown,
        )
    };
    document.set_every_support(id, kept);
    document.set_transform(id, transform);
    if let Some(shell) = shell {
        document.set_built(id, BuiltCavity::of(&shell), shell.mesh);
    }
    Ok(())
}
