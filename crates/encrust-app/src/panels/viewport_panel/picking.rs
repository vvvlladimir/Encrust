//! What the cursor is over: the model a click selects, the face it lays down or washes, the
//! corner a measurement snaps to, and the part of a support a ray meets.

use core_supports::{Grab, Profiles, Slack, SupportTree, axis_of, grab_along};

use core_geometry::{Aabb, PlacedHit, Ray, Vec3, ray_aabb};

use crate::camera::OrbitCamera;
use crate::measure;
use crate::panels::{Window, section};
use crate::pick::{
    Printed, pick, pick_material, pick_printed, pick_surface, ray_through, seen_along,
};
use crate::scene::{ObjectId, Scene, SceneObject};

/// The model under the cursor through the section this frame shows; see [`pick`].
pub(super) fn object_under(
    window: &Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
) -> Option<ObjectId> {
    let (scene, camera) = (&window.doc.scene, &window.view.camera);
    pick(scene, camera, viewport, cursor, section::shown(window))
}

/// The printed surface under the cursor through the section this frame shows, a model's or
/// its supports'; see [`pick_printed`].
pub(super) fn printed_under(
    window: &Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
) -> Option<(ObjectId, Printed, PlacedHit)> {
    let (scene, camera) = (&window.doc.scene, &window.view.camera);
    pick_printed(scene, camera, viewport, cursor, section::shown(window))
}

/// The drawn face under the cursor through the section this frame shows; see [`pick_surface`].
pub(super) fn surface_under(
    window: &Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
) -> Option<(ObjectId, PlacedHit)> {
    let (scene, camera) = (&window.doc.scene, &window.view.camera);
    pick_surface(scene, camera, viewport, cursor, section::shown(window))
}

/// The face or cap under the cursor through the section this frame shows; see
/// [`pick_material`].
pub(super) fn material_under(
    window: &Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
) -> Option<(ObjectId, PlacedHit)> {
    let (scene, camera) = (&window.doc.scene, &window.view.camera);
    pick_material(scene, camera, viewport, cursor, section::shown(window))
}

/// The point a scroll zooms towards: the drawn face or cut cap under the cursor, else where
/// its line of sight meets the plate, else `None`, and the zoom closes on the target.
pub(super) fn zoom_anchor(
    window: &Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
) -> Option<Vec3> {
    if let Some((_, hit)) = material_under(window, viewport, cursor) {
        return Some(hit.point);
    }
    let ray = ray_through(&window.view.camera, viewport, cursor)?;
    let plate = &window.doc.plate;
    let footprint = Aabb::new(Vec3::ZERO, Vec3::new(plate.x_mm, plate.y_mm, 0.0));
    ray_aabb(&ray, &footprint).map(|t| ray.at(t))
}

/// Takes the flat face the click landed on, or the point on a model or a support, snapped
/// the way the tool is set unless `alt` is held.
pub(super) fn measure_under_cursor(
    window: &mut Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
    alt: bool,
) {
    let surface = surface_under(window, viewport, cursor);
    if window.tools.measure.target == measure::Target::Planes {
        let Some((object, hit)) = on_model(&window.doc.scene, surface) else {
            return;
        };
        let adjacency = window.tools.measure_edges.of(&object.mesh);
        if let Some(plane) = measure::Plane::of(object, &adjacency, hit.face) {
            window.tools.measure.pick_plane(plane);
        }
        return;
    }
    let snap = window.tools.measure.snap_now(alt);
    let support = support_point_under(window, viewport, cursor, snap)
        // Free on the model a tip stands in, unless the support is in front of it.
        .filter(|(_, along_mm)| !alt || surface.is_none_or(|(_, hit)| *along_mm < hit.distance));
    if let Some(((point, snapped), _)) = support {
        window.tools.measure.pick(point, snapped, None);
        return;
    }
    let Some((object, hit)) = on_model(&window.doc.scene, surface) else {
        return;
    };
    let adjacency = window.tools.measure_edges.of(&object.mesh);
    let reach_mm = reach_at(&window.view.camera, viewport, cursor, hit.point, SNAP_PX);
    let (point, snapped) = measure::snap(
        &object.mesh,
        &object.bvh,
        &adjacency,
        object.transform,
        hit.point,
        reach_mm,
        snap,
    );
    let anchor = measure::Anchor::of(object, point);
    window.tools.measure.pick(point, snapped, anchor);
}

/// The model `surface` was met on, with where.
fn on_model(
    scene: &Scene,
    surface: Option<(ObjectId, PlacedHit)>,
) -> Option<(&SceneObject, PlacedHit)> {
    surface.and_then(|(id, hit)| Some((scene.get(id)?, hit)))
}

/// The point of the support part under the cursor a measurement takes, and how far along
/// the cursor's ray the part was met, mm.
fn support_point_under(
    window: &Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
    snap: measure::Snap,
) -> Option<((Vec3, measure::Snapped), f32)> {
    let Aimed {
        id,
        hit,
        at,
        along_mm,
    } = grab_under_cursor(window, viewport, cursor)?;
    let object = window.doc.scene.get(id)?;
    let tree = object.supports.trees().get(hit.tree)?;
    let table = window.tools.supports.table_for(object);
    let profile = Profiles::new(&table)?.of(tree.group());
    let axis = axis_of(tree, profile, hit.part)?;
    Some((measure::snap_to_support(axis, at, snap), along_mm))
}

/// How near the cursor a corner or an edge catches a click, points of the screen.
const SNAP_PX: f32 = 14.0;

/// How near the cursor a part of a support is caught, points of the screen: a pillar a point
/// wide is hit without care, and a part 6 points off is plainly not the one under the cursor.
const AIM_PX: f32 = 6.0;

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
    let Some((id, hit)) = surface_under(window, viewport, cursor) else {
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

/// Marks the point under the cursor as the part the Cut tool's Split cuts off, and picks
/// the model it is on. A click on empty plate keeps the tool waiting.
pub(super) fn mark_part_under_cursor(
    window: &mut Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
) {
    let Some((id, hit)) = surface_under(window, viewport, cursor) else {
        return;
    };
    let Some(object) = window.doc.scene.get(id) else {
        return;
    };
    window.tools.cut.mark_part(object, hit.point);
    window.doc.scene.select(Some(id));
}

/// Keeps the flat face under the cursor as the one a click would lay down.
pub(super) fn point_at_face(window: &mut Window, viewport: egui::Rect, cursor: Option<egui::Pos2>) {
    let hit = cursor.and_then(|cursor| surface_under(window, viewport, cursor));
    match hit.and_then(|(id, hit)| Some((window.doc.scene.get(id)?, hit.face))) {
        Some((object, face)) => window.tools.orient.point_at(object, face),
        None => window.tools.orient.facet = None,
    }
}

/// A click that was not a drag picks whatever is under it, or clears the selection.
/// Held with a modifier it adds to what is picked, or takes that one back out.
pub(super) fn select_under_cursor(
    window: &mut Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
    adding: bool,
) {
    let hit = object_under(window, viewport, cursor);
    let scene = &mut window.doc.scene;
    match (adding, hit) {
        (true, Some(id)) => scene.toggle_selected(id),
        (true, None) => {}
        (false, hit) => scene.select(hit),
    }
}

/// Whether this click only picks the model it landed on (outside the selection), so the next
/// click acts on it (`docs/decisions/0193`). With nothing picked the click goes through (ADR 0101).
pub(super) fn takes_the_pick(
    window: &mut Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
) -> bool {
    let hit = object_under(window, viewport, cursor);
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

/// How far past the model's surface along the ray a part may be met and still be in front,
/// millimetres: parts are met on their axis, and a tip is sunk into its surface.
const BEHIND_MM: f32 = 1.0;

/// How far a ray through `cursor` may pass a part of a support and still take it: [`AIM_PX`]
/// of screen at whatever depth the part stands, measured where the camera looks and twice as far.
pub(super) fn aim_slack(camera: &OrbitCamera, viewport: egui::Rect, cursor: egui::Pos2) -> Slack {
    let Some(ray) = ray_through(camera, viewport, cursor) else {
        return Slack::default();
    };
    let depth_mm = (camera.target - ray.origin).dot(ray.direction).max(1.0);
    let reach = |along_mm: f32| reach_at(camera, viewport, cursor, ray.at(along_mm), AIM_PX);
    let (near_mm, far_mm) = (reach(depth_mm), reach(2.0 * depth_mm));
    let per_mm = (far_mm - near_mm) / depth_mm;
    Slack {
        mm: near_mm - per_mm * depth_mm,
        per_mm,
    }
}

/// A part of a support the cursor is on: the object, which part, and the point of the cursor's
/// ray at that part, plate mm, which a drag keeps under the cursor.
pub(super) struct Aimed {
    pub id: crate::scene::ObjectId,
    pub hit: Grab,
    pub at: Vec3,
    /// How far along the cursor's ray `at` lies, mm.
    pub along_mm: f32,
}

/// Which part of which support the cursor is on. Objects are searched in the order they were
/// imported.
pub(super) fn grab_under_cursor(
    window: &Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
) -> Option<Aimed> {
    let ray = ray_through(&window.view.camera, viewport, cursor)?;
    let slack = aim_slack(&window.view.camera, viewport, cursor);

    // Only what is seen can be aimed at: not behind a model, cut away or in a hidden group,
    // but inside a cavity the section opens.
    let (near_mm, wall_mm) = seen_along(&window.doc.scene, &ray, section::shown(window))?;
    window.doc.scene.here().find_map(|object| {
        let table = window.tools.supports.table_for(object);
        let profiles = Profiles::new(&table)?;
        let hidden = window.tools.supports.hidden();
        let (hit, along_mm) = grab_shown(object.supports.trees(), &hidden, &ray, profiles, slack)?;
        (near_mm - BEHIND_MM..=wall_mm + BEHIND_MM)
            .contains(&along_mm)
            .then_some(Aimed {
                id: object.id,
                hit,
                at: ray.at(along_mm),
                along_mm,
            })
    })
}

/// What `grab_along` answers among the trees whose group is not `hidden`, numbered as
/// `trees` numbers them: a support left out of the view is not in the way of one behind.
fn grab_shown(
    trees: &[SupportTree],
    hidden: &[bool],
    ray: &Ray,
    profiles: Profiles,
    slack: Slack,
) -> Option<(Grab, f32)> {
    let shown: Vec<usize> = (0..trees.len())
        .filter(|tree| {
            !hidden
                .get(trees[*tree].group() as usize)
                .is_some_and(|out| *out)
        })
        .collect();
    if shown.len() == trees.len() {
        return grab_along(trees, ray, profiles, slack);
    }
    let visible: Vec<SupportTree> = shown.iter().map(|tree| trees[*tree].clone()).collect();
    let (hit, along_mm) = grab_along(&visible, ray, profiles, slack)?;
    let tree = *shown.get(hit.tree)?;
    Some((Grab { tree, ..hit }, along_mm))
}

#[cfg(test)]
mod tests;
