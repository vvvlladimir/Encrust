//! The Hollow tool's clicks: the blockers that keep a wall solid, the drain holes drilled
//! into the surface and the channels dug between them, and what each would put down.

use std::sync::Arc;

use core_geometry::{PlacedHit, Scalar, Transform, Vec3};
use core_volume::{ModelHollow, PENDING_MARKER_MM, markers};

use crate::panels::Window;
use crate::render::{Ghost, GhostShape};
use crate::scene::ObjectId;
use crate::ui::theme;

use super::picking::{material_under, surface_under};

/// Keeps the wall solid where the model was clicked, and selects what it was put on so
/// that the inspector is talking about the same object.
pub(super) fn place_blocker_under_cursor(
    window: &mut Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
) {
    let Some((id, hit)) = material_under(window, viewport, cursor) else {
        return;
    };
    window.doc.scene.select(Some(id));
    let radius_mm = window.tools.hollow.state.blocker_mm;
    if let Some(object) = window.doc.scene.get_mut(id) {
        let transform = object.transform;
        object.hollow.add_blocker(hit.point, radius_mm, transform);
    }
}

/// Takes away the blocker the click landed in.
pub(super) fn remove_blocker_under_cursor(
    window: &mut Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
) {
    let Some((id, hit)) = material_under(window, viewport, cursor) else {
        return;
    };
    if let Some(object) = window.doc.scene.get_mut(id) {
        let transform = object.transform;
        object.hollow.remove_blocker(hit.point, transform);
    }
}

/// Drills a drain hole where the model was clicked, straight into the surface under the
/// cursor, and selects what it was drilled into.
pub(super) fn place_drain_under_cursor(
    window: &mut Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
) {
    let Some((id, hit)) = surface_under(window, viewport, cursor) else {
        return;
    };
    window.doc.scene.select(Some(id));
    window.tools.drain.stale();
    if let Some(object) = window.doc.scene.get_mut(id) {
        let size = window.tools.drain.size_for(object);
        let transform = object.transform;
        object.hollow.add_drain(
            &object.mesh,
            &object.bvh,
            hit.point,
            hit.normal,
            size,
            transform,
        );
    }
}

/// Adds a point to the channel being laid out on the model under the cursor.
pub(super) fn add_channel_point_under_cursor(
    window: &mut Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
) {
    let Some((id, hit)) = surface_under(window, viewport, cursor) else {
        return;
    };
    window.doc.scene.select(Some(id));
    if let Some(object) = window.doc.scene.get_mut(id) {
        let transform = object.transform;
        object
            .hollow
            .add_channel_point(hit.point, hit.normal, transform);
    }
}

/// Takes away the drain hole the click landed in.
pub(super) fn remove_drain_under_cursor(
    window: &mut Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
) {
    let Some((id, hit)) = surface_under(window, viewport, cursor) else {
        return;
    };
    window.tools.drain.stale();
    if let Some(object) = window.doc.scene.get_mut(id) {
        let transform = object.transform;
        object.hollow.remove_drain(hit.point, transform);
    }
}

/// The blocker a click at `hit` on object `id` would put down.
pub(super) fn blocker_ghost(window: &Window, id: ObjectId, hit: &PlacedHit) -> Option<Ghost> {
    let object = window.doc.scene.get(id)?;
    let radius_mm = window.tools.hollow.state.blocker_mm;
    let blocker = ModelHollow::blocker_at(hit.point, radius_mm, object.transform)?;
    marker_ghost(blocker.from, blocker.radius_mm, object.transform)
}

/// The drain hole a click at `hit` on object `id` would drill, as deep as it would be cut.
pub(super) fn drain_ghost(window: &Window, id: ObjectId, hit: &PlacedHit) -> Option<Ghost> {
    let object = window.doc.scene.get(id)?;
    let size = window.tools.drain.size_for(object);
    let hole = ModelHollow::drain_at(
        &object.mesh,
        &object.bvh,
        hit.point,
        hit.normal,
        size,
        object.transform,
    )?;
    Some(Ghost {
        shape: GhostShape::Hole {
            id,
            hole: object.hollow.as_cut(hole),
        },
        colour: theme::colors().accent,
    })
}

/// The point a click at `hit` on object `id` would add to the channel being laid out.
pub(super) fn channel_point_ghost(window: &Window, id: ObjectId, hit: &PlacedHit) -> Option<Ghost> {
    let object = window.doc.scene.get(id)?;
    let (point, _) = ModelHollow::channel_point_at(hit.point, hit.normal, object.transform)?;
    marker_ghost(point, PENDING_MARKER_MM, object.transform)
}

/// A ball about `center` in the model's own space, drawn the way placed markers are.
fn marker_ghost(center: Vec3, radius_mm: Scalar, transform: Transform) -> Option<Ghost> {
    Some(Ghost {
        shape: GhostShape::Body {
            mesh: Arc::new(markers([(center, radius_mm)])?),
            transform,
        },
        colour: theme::scene().blocker,
    })
}
