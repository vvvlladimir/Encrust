//! What the cursor is over: the model a click selects, the face it lays down or washes, the
//! corner a measurement snaps to, and the part of a support a ray meets.

use core_supports::{Grab, Profiles, grab};

use core_geometry::Vec3;

use crate::camera::OrbitCamera;
use crate::measure;
use crate::panels::Window;
use crate::pick::{pick, pick_surface, ray_through};
use crate::scene::{ObjectId, Scene};

/// Takes the flat face the click landed on, or the point, snapped the way the tool is set
/// unless `alt` is held.
pub(super) fn measure_under_cursor(
    window: &mut Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
    alt: bool,
) {
    let Some((id, hit)) = pick_surface(&window.doc.scene, &window.view.camera, viewport, cursor)
    else {
        return;
    };
    let Some(object) = window.doc.scene.get(id) else {
        return;
    };
    let adjacency = window.tools.measure_edges.of(&object.mesh);
    let measure = &mut window.tools.measure;
    if measure.target == measure::Target::Planes {
        if let Some(plane) = measure::Plane::of(object, &adjacency, hit.face) {
            measure.pick_plane(plane);
        }
        return;
    }
    let reach_mm = reach_at(&window.view.camera, viewport, cursor, hit.point, SNAP_PX);
    let (point, snapped) = measure::snap(
        &object.mesh,
        &object.bvh,
        &adjacency,
        object.transform,
        hit.point,
        reach_mm,
        measure.snap_now(alt),
    );
    measure.pick(point, snapped);
}

/// How near the cursor a corner or an edge catches a click, points of the screen.
const SNAP_PX: f32 = 14.0;

/// How near the cursor a part of a support is caught, points of the screen: enough that
/// a pillar a point or two wide can be hit, too little to take one the cursor is not on.
const AIM_PX: f32 = 2.0;

/// `points` of the screen as millimetres of the plate at `at`, which the cursor lies
/// over: a reach is as far on screen however near the model is.
fn reach_at(
    camera: &OrbitCamera,
    viewport: egui::Rect,
    cursor: egui::Pos2,
    at: Vec3,
    points: f32,
) -> f32 {
    let beside = cursor + egui::vec2(points, 0.0);
    let (Some(here), Some(there)) = (
        ray_through(camera, viewport, cursor),
        ray_through(camera, viewport, beside),
    ) else {
        return 0.0;
    };
    let facing = there.direction.dot(here.direction);
    if facing <= f32::EPSILON {
        return 0.0;
    }
    // Both rays cut by the plane through `at` that faces the camera.
    let across =
        there.origin + there.direction * ((at - there.origin).dot(here.direction) / facing);
    (across - at).length()
}

/// Lays the face under the cursor on the plate, and picks the model it belongs to. A
/// click on empty plate keeps the tool waiting for a face.
pub(super) fn lay_face_under_cursor(window: &mut Window, viewport: egui::Rect, cursor: egui::Pos2) {
    let Some((id, hit)) = pick_surface(&window.doc.scene, &window.view.camera, viewport, cursor)
    else {
        return;
    };
    let Some(object) = window.doc.scene.get_mut(id) else {
        return;
    };
    window.tools.orient.point_at(object, hit.face);
    let normal = window
        .tools
        .orient
        .facet
        .as_ref()
        .map_or(hit.normal, |facet| object.world_normal(facet.normal));
    if object.lay_face_down(normal) {
        window.doc.scene.select(Some(id));
        window.tools.orient.stop_picking();
    }
}

/// Keeps the flat face under the cursor as the one a click would lay down.
pub(super) fn point_at_face(window: &mut Window, viewport: egui::Rect, cursor: Option<egui::Pos2>) {
    let hit = cursor
        .and_then(|cursor| pick_surface(&window.doc.scene, &window.view.camera, viewport, cursor));
    match hit.and_then(|(id, hit)| Some((window.doc.scene.get(id)?, hit.face))) {
        Some((object, face)) => window.tools.orient.point_at(object, face),
        None => window.tools.orient.facet = None,
    }
}

/// A click that was not a drag picks whatever is under it, or clears the selection.
/// Held with a modifier it adds to what is picked, or takes that one back out.
pub(super) fn select_under_cursor(
    scene: &mut Scene,
    camera: &OrbitCamera,
    viewport: egui::Rect,
    cursor: egui::Pos2,
    adding: bool,
) {
    let hit = pick(scene, camera, viewport, cursor);
    match (adding, hit) {
        (true, Some(id)) => scene.toggle_selected(id),
        (true, None) => {}
        (false, hit) => scene.select(hit),
    }
}

/// Whether this click only picks the model it landed on, which is what a click outside
/// the selection does: a tool works on what is picked, so the first click on another part
/// changes what is aimed at and the next one acts on it. See `docs/decisions/0193`.
///
/// With nothing picked a tool has the whole plate (ADR 0101), so there is nothing to aim
/// and the click goes through.
pub(super) fn takes_the_pick(
    window: &mut Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
) -> bool {
    let hit = pick(&window.doc.scene, &window.view.camera, viewport, cursor);
    if !aims_elsewhere(&window.doc.scene, hit) {
        return false;
    }
    window.doc.scene.select(hit);
    true
}

/// Whether `hit` is a model the tools are not working on while they are working on
/// something.
pub(super) fn aims_elsewhere(scene: &Scene, hit: Option<ObjectId>) -> bool {
    hit.is_some_and(|id| scene.has_selection() && !scene.is_selected(id))
}

/// How far a ray through `cursor` may pass a part of a support and still take it,
/// millimetres, measured on that ray as far off as the camera is looking.
pub(super) fn aim_slack_mm(camera: &OrbitCamera, viewport: egui::Rect, cursor: egui::Pos2) -> f32 {
    let Some(ray) = ray_through(camera, viewport, cursor) else {
        return 0.0;
    };
    let under = ray.at((camera.target - ray.origin).dot(ray.direction));
    reach_at(camera, viewport, cursor, under, AIM_PX)
}

/// Which part of which support the cursor is on, with the object it belongs to. Objects are searched in the order they were imported.
pub(super) fn grab_under_cursor(
    window: &Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
) -> Option<(crate::scene::ObjectId, Grab)> {
    let ray = ray_through(&window.view.camera, viewport, cursor)?;
    let table = window.tools.supports.table();
    let profiles = Profiles::new(&table)?;
    let slack_mm = aim_slack_mm(&window.view.camera, viewport, cursor);

    window.doc.scene.here().find_map(|object| {
        grab(object.supports.trees(), &ray, profiles, slack_mm).map(|it| (object.id, it))
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[test]
    fn the_aim_reaches_as_far_wherever_on_the_screen_the_cursor_is() {
        let camera = OrbitCamera::default();
        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));

        // 600 points of height show 2 * 300 mm * tan(0.4) = 254 mm, so a point is
        // 0.42 mm where the camera looks and two of them 0.85 mm.
        let middle = aim_slack_mm(&camera, viewport, viewport.center());
        assert!(
            (middle - 0.85).abs() < 0.05,
            "two points at the middle of the view are 0.85 mm, got {middle}"
        );
        let corner = aim_slack_mm(&camera, viewport, egui::pos2(40.0, 40.0));
        assert!(
            corner < 2.0 * middle,
            "a cursor far from the middle reaches no further, got {corner} mm"
        );
    }

    #[test]
    fn a_click_outside_the_selection_only_aims_the_tool() {
        let mut scene = Scene::default();
        let cube = Arc::new(core_geometry::Mesh::new(
            vec![Vec3::ZERO, Vec3::X, Vec3::Y],
            vec![[0, 1, 2]],
        ));
        let summary = crate::scene::ImportSummary {
            vertices_merged: 0,
            faces_removed: 0,
            orientation: core_geometry::Orientation {
                flipped_faces: 0,
                inverted_shells: 0,
                orientable: true,
            },
            diagnostics: core_geometry::diagnose(&cube),
        };
        let first = scene.insert(crate::scene::Imported::new(
            "first".to_owned(),
            Arc::clone(&cube),
            core_geometry::Transform::default(),
            summary.clone(),
        ));
        let second = scene.insert(crate::scene::Imported::new(
            "second".to_owned(),
            cube,
            core_geometry::Transform::default(),
            summary,
        ));

        scene.select(Some(first));
        assert!(
            aims_elsewhere(&scene, Some(second)),
            "the click lands on a model the tool is not working on"
        );
        assert!(
            !aims_elsewhere(&scene, Some(first)),
            "a second click on what is picked is the tool's"
        );
        assert!(
            !aims_elsewhere(&scene, None),
            "a click on empty plate is not an aim"
        );

        scene.clear_selection();
        assert!(
            !aims_elsewhere(&scene, Some(second)),
            "with nothing picked the tool has the whole plate"
        );
    }
}
