//! The Hollow tool's clicks: the blockers that keep a wall solid, the drain holes drilled
//! into the surface and the channels dug between them.

use crate::panels::Window;
use crate::pick::pick_surface;

/// Keeps the wall solid where the model was clicked, and selects what it was put on so
/// that the inspector is talking about the same object.
pub(super) fn place_blocker_under_cursor(
    window: &mut Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
) {
    let Some((id, hit)) = pick_surface(&window.doc.scene, &window.view.camera, viewport, cursor)
    else {
        return;
    };
    window.doc.scene.select(Some(id));
    let radius_mm = window.tools.hollow.blocker_mm;
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
    let Some((id, hit)) = pick_surface(&window.doc.scene, &window.view.camera, viewport, cursor)
    else {
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
    let Some((id, hit)) = pick_surface(&window.doc.scene, &window.view.camera, viewport, cursor)
    else {
        return;
    };
    window.doc.scene.select(Some(id));
    let size = window.tools.drain.size();
    window.tools.drain.stale();
    if let Some(object) = window.doc.scene.get_mut(id) {
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
    let Some((id, hit)) = pick_surface(&window.doc.scene, &window.view.camera, viewport, cursor)
    else {
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
    let Some((id, hit)) = pick_surface(&window.doc.scene, &window.view.camera, viewport, cursor)
    else {
        return;
    };
    window.tools.drain.stale();
    if let Some(object) = window.doc.scene.get_mut(id) {
        let transform = object.transform;
        object.hollow.remove_drain(hit.point, transform);
    }
}
