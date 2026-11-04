//! The Hollow tool's clicks: the blockers that keep a wall solid, the drain holes drilled
//! into the surface and the channels dug between them, and what each would put down.

use std::sync::Arc;

use core_geometry::{Bvh, Mesh, PlacedHit, Transform};
use core_volume::{Blocker, DrainHole, ModelHollow, PENDING_MARKER_MM, across_mm, blocker_markers};

use crate::panels::Window;
use crate::panels::inspector::drain::rebuild_around_cuts;
use crate::pick::{Printed, ray_through};
use crate::render::{Ghost, GhostShape};
use crate::scene::{ObjectId, SceneObject};
use crate::ui::theme;

use super::picking::{material_under, printed_under, surface_under};

/// Keeps the wall solid where the model was clicked, with `rod` on from the last blocker,
/// and selects what it was put on so that the inspector is talking about the same object.
pub(super) fn place_blocker_under_cursor(
    window: &mut Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
    rod: bool,
) {
    let Some((id, hit)) = material_under(window, viewport, cursor) else {
        return;
    };
    window.doc.scene.select(Some(id));
    let Some(object) = window.doc.scene.get(id) else {
        return;
    };
    let Some(blocker) = window.tools.hollow.blocker_for(object, &hit, rod) else {
        return;
    };
    if let Some(object) = window.doc.scene.get_mut(id) {
        object.hollow.place_blocker(blocker);
    }
}

/// Takes away the blocker the click went through, deep in the model or not.
pub(super) fn remove_blocker_under_cursor(
    window: &mut Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
) {
    let Some((id, hit)) = material_under(window, viewport, cursor) else {
        return;
    };
    let Some(ray) = ray_through(&window.view.camera, viewport, cursor) else {
        return;
    };
    if let Some(object) = window.doc.scene.get_mut(id) {
        let transform = object.transform;
        // Halfway across the part: deep enough for a fitted ball, short of its far side's.
        let across = across_mm(
            &object.mesh,
            &object.bvh,
            transform,
            hit.point,
            ray.direction,
        );
        let reach_mm = (hit.point - ray.origin).dot(ray.direction) + across.unwrap_or(0.0) / 2.0;
        object.hollow.remove_blocker(&ray, reach_mm, transform);
    }
}

/// Drills a drain hole straight into the printed surface under the cursor — a model's, an
/// earlier hole's floor or wall among it, or its supports' — and selects the model.
pub(super) fn place_drain_under_cursor(
    window: &mut Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
) {
    let Some((id, printed, hit)) = printed_under(window, viewport, cursor) else {
        return;
    };
    window.doc.scene.select(Some(id));
    window.tools.drain.stale();
    let hole = window
        .doc
        .scene
        .get(id)
        .and_then(|object| hole_under(window, object, printed, &hit));
    if let (Some(hole), Some(object)) = (hole, window.doc.scene.get_mut(id)) {
        object.hollow.add_hole(hole);
    }
}

/// The hole a click at `hit` on `printed` of `object` drills, in the model's own space.
fn hole_under(
    window: &Window,
    object: &SceneObject,
    printed: Printed,
    hit: &PlacedHit,
) -> Option<DrainHole> {
    let size = window.tools.drain.size();
    let (point, normal, transform) = (hit.point, hit.normal, object.transform);
    match printed {
        Printed::Model => {
            let (surface, bvh) = object.support_surface();
            ModelHollow::drain_at(surface, bvh, point, normal, size, transform)
        }
        Printed::Supports(group) => {
            let (mesh, bvh) = support_group(object, group)?;
            ModelHollow::support_drain_at(&mesh, &bvh, point, normal, size, transform)
        }
    }
}

/// Support group `group` of `object` as it prints, and its hierarchy, once built.
fn support_group(object: &SceneObject, group: usize) -> Option<(Arc<Mesh>, Arc<Bvh>)> {
    let (groups, bvhs) = object.supports_built()?;
    Some((
        Arc::clone(&groups.get(group)?.mesh),
        Arc::clone(bvhs.get(group)?),
    ))
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
    let Some((id, _, hit)) = printed_under(window, viewport, cursor) else {
        return;
    };
    window.tools.drain.stale();
    if let Some(object) = window.doc.scene.get_mut(id) {
        let transform = object.transform;
        object.hollow.remove_drain(hit.point, transform);
    }
    rebuild_around_cuts(window);
}

/// The blocker a click at `hit` on object `id` would put down, with `rod` run on from the
/// last one.
pub(super) fn blocker_ghost(
    window: &Window,
    id: ObjectId,
    hit: &PlacedHit,
    rod: bool,
) -> Option<Ghost> {
    let object = window.doc.scene.get(id)?;
    let blocker = window.tools.hollow.blocker_for(object, hit, rod)?;
    marker_ghost(blocker, object.transform)
}

/// The drain hole a click at `hit` on `printed` of object `id` would drill, as deep as it
/// would be cut; none inside a hole already drilled that takes it in.
pub(super) fn drain_ghost(
    window: &Window,
    id: ObjectId,
    printed: Printed,
    hit: &PlacedHit,
) -> Option<Ghost> {
    let object = window.doc.scene.get(id)?;
    let hole = hole_under(window, object, printed, hit)
        .filter(|hole| !object.hollow.has_hole_over(hole))?;
    let into = match printed {
        Printed::Model => None,
        Printed::Supports(group) => Some(support_group(object, group)?.0),
    };
    Some(Ghost {
        shape: GhostShape::Hole {
            id,
            hole: object.hollow.as_cut(hole),
            into,
        },
        colour: theme::colors().accent,
    })
}

/// The point a click at `hit` on object `id` would add to the channel being laid out.
pub(super) fn channel_point_ghost(window: &Window, id: ObjectId, hit: &PlacedHit) -> Option<Ghost> {
    let object = window.doc.scene.get(id)?;
    let (point, _) = ModelHollow::channel_point_at(hit.point, hit.normal, object.transform)?;
    marker_ghost(Blocker::ball(point, PENDING_MARKER_MM), object.transform)
}

/// `blocker`, in the model's own space, drawn the way placed markers are.
fn marker_ghost(blocker: Blocker, transform: Transform) -> Option<Ghost> {
    Some(Ghost {
        shape: GhostShape::Body {
            mesh: Arc::new(blocker_markers([blocker])?),
            transform,
        },
        colour: theme::scene().blocker,
    })
}
