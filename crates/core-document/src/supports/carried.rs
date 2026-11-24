use core_edit::ObjectId;
use core_geometry::{Bvh, Transform};

use super::groups::table_of;
use super::kept_supports;
use crate::document::Document;

/// Stands object `id` at `transform` with its supports carried there as the window's are: frozen
/// trees replanted or thawed (ADR 0439), grown trees dropped to grow again where it now stands.
pub(crate) fn stand_at(document: &mut Document, id: ObjectId, transform: Transform) {
    let Some((state, meshes)) = document.object(id) else {
        return;
    };
    if state.transform == transform {
        return;
    }
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
}
