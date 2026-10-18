//! What the cursor is over: the model a click selects, the face it lays down or washes, the
//! corner a measurement snaps to, and the part of a support a ray meets.

use core_supports::{Grab, Profiles, SupportTree, grab_along};

use core_geometry::{PlacedHit, Ray, Vec3, raycast_placed};

use crate::camera::OrbitCamera;
use crate::measure;
use crate::panels::{Window, section};
use crate::pick::{pick, pick_material, pick_surface, ray_through};
use crate::scene::{ObjectId, Scene};

/// The model under the cursor through the section this frame shows; see [`pick`].
pub(super) fn object_under(
    window: &Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
) -> Option<ObjectId> {
    let (scene, camera) = (&window.doc.scene, &window.view.camera);
    pick(scene, camera, viewport, cursor, section::shown(window))
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

/// Takes the flat face the click landed on, or the point, snapped the way the tool is set
/// unless `alt` is held.
pub(super) fn measure_under_cursor(
    window: &mut Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
    alt: bool,
) {
    let Some((id, hit)) = surface_under(window, viewport, cursor) else {
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
    window.tools.cut.mark_part(id, object, hit.point);
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

/// How much further along the ray than the model's surface a part may be met and still
/// be in front of it, millimetres: a part is met on its axis, half its width behind the
/// side that faces the camera, and a tip is sunk into the surface it holds.
const BEHIND_MM: f32 = 1.0;

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
    let slack_mm = aim_slack_mm(&window.view.camera, viewport, cursor);

    // A support behind a model is not under the cursor, and one of a group left out of
    // the view is not there to be aimed at.
    let wall_mm = window
        .doc
        .scene
        .here()
        .filter_map(|object| raycast_placed(&object.mesh, &object.bvh, object.transform, &ray))
        .map(|hit| hit.distance)
        .fold(f32::INFINITY, f32::min);
    window.doc.scene.here().find_map(|object| {
        let table = window.tools.supports.table_for(object);
        let profiles = Profiles::new(&table)?;
        let hidden = window.tools.supports.hidden();
        let (hit, along_mm) =
            grab_shown(object.supports.trees(), &hidden, &ray, profiles, slack_mm)?;
        (along_mm <= wall_mm + BEHIND_MM).then_some((object.id, hit))
    })
}

/// What `grab_along` answers among the trees whose group is not `hidden`, numbered as
/// `trees` numbers them: a support left out of the view is not in the way of one behind.
fn grab_shown(
    trees: &[SupportTree],
    hidden: &[bool],
    ray: &Ray,
    profiles: Profiles,
    slack_mm: f32,
) -> Option<(Grab, f32)> {
    let shown: Vec<usize> = (0..trees.len())
        .filter(|tree| {
            !hidden
                .get(trees[*tree].group() as usize)
                .is_some_and(|out| *out)
        })
        .collect();
    if shown.len() == trees.len() {
        return grab_along(trees, ray, profiles, slack_mm);
    }
    let visible: Vec<SupportTree> = shown.iter().map(|tree| trees[*tree].clone()).collect();
    let (hit, along_mm) = grab_along(&visible, ray, profiles, slack_mm)?;
    let tree = *shown.get(hit.tree)?;
    Some((Grab { tree, ..hit }, along_mm))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[test]
    fn a_hidden_support_is_not_in_the_way_of_the_one_behind_it() {
        let table = vec![printer_profiles::SupportProfile::default(); 2];
        let mut supports = core_supports::ModelSupports::default();
        supports.add(
            Vec3::new(0.0, 0.0, 20.0),
            core_geometry::Transform::default(),
            0,
        );
        supports.add(
            Vec3::new(30.0, 0.0, 20.0),
            core_geometry::Transform::default(),
            1,
        );
        let nothing = core_geometry::Mesh::default();
        let bvh = core_geometry::Bvh::build(&nothing);
        supports.refresh(&nothing, &bvh, core_geometry::Transform::default(), &table);
        let trees = supports.trees();
        let profiles = Profiles::new(&table).expect("two profiles");
        // Level with the middle of both pillars, the first group's nearer the eye.
        let ray = Ray::new(Vec3::new(-50.0, 0.0, 10.0), Vec3::X);
        let group_met = |hidden: &[bool]| {
            grab_shown(trees, hidden, &ray, profiles, 0.0).map(|(hit, _)| trees[hit.tree].group())
        };

        assert_eq!(group_met(&[false, false]), Some(0), "the nearer one");
        assert_eq!(group_met(&[true, false]), Some(1), "the one behind it");
        assert_eq!(group_met(&[true, true]), None);
    }

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
