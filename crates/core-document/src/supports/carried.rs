use core_edit::ObjectId;
use core_engine::project::supports_of;
use core_geometry::{Bvh, Transform};

use super::table_of;
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
        let (source, drains) = (&meshes.source, &state.hollow.drains);
        let mut supports = supports_of(
            &state.supports,
            drains,
            source,
            surface,
            &bvh,
            state.transform,
            &table,
        );
        supports.refresh_on(source, surface, &bvh, transform, &table);
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
