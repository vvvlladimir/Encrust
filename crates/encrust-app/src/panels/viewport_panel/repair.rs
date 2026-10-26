//! What the Repair tool marks over the models, where it sends the camera, and the open
//! edge a click in its Faces mode picks.

use crate::highlight::{EdgeMark, Highlight, Spot};
use crate::panels::Window;
use crate::repair::{Picking, RepairMode};
use crate::ui::theme;

use super::overlays::projector;

/// Points from the cursor an open edge may pass and still be the one a click picks.
const EDGE_REACH_POINTS: f32 = 10.0;

/// Open edges past which the one under the pointer is not looked for: an unwelded model's
/// every edge is open, and looking over a million of them a frame would stall the window.
const HOVER_EDGES: usize = 200_000;

/// How far behind the surface under the cursor an edge may lie and still be taken, as a
/// share of the camera's distance: the rim of a hole is that surface's own edge.
const BEHIND_SHARE: f32 = 0.02;

/// The Model mode's findings and picked shells, or the Faces mode's picks over the open
/// edges they are picked among.
pub(super) fn marked(
    window: &Window,
    viewport: egui::Rect,
    cursor: Option<egui::Pos2>,
) -> Highlight {
    let (scene, repairs) = (&window.doc.scene, &window.doc.repairs);
    let tool = &window.tools.repair;
    let mut highlight = match tool.mode {
        RepairMode::Model => repairs.found.highlight(scene),
        RepairMode::Faces => repairs.hand.highlight(scene),
    };
    let more = match (tool.mode, tool.picking) {
        (RepairMode::Model, _) => repairs.shells.highlight(scene),
        (RepairMode::Faces, Picking::Edges) => open_edges(window),
        (RepairMode::Faces, Picking::Faces) => Highlight::default(),
    };
    // The open edges go under the picked ones, which are drawn over them in their colour.
    highlight.faces.extend(more.faces);
    highlight.edges.splice(0..0, more.edges);
    if (tool.mode, tool.picking) == (RepairMode::Faces, Picking::Edges) {
        let cursor = cursor.filter(|at| viewport.contains(*at));
        highlight
            .edges
            .extend(hovered(window, viewport, cursor).edges);
    }
    highlight
}

/// Where the camera is to go: the finding or the shell the last press went to.
pub(super) fn flight(window: &mut Window) -> Option<Spot> {
    let repairs = &mut window.doc.repairs;
    let found = repairs.found.take_flight(&window.doc.scene);
    found.or_else(|| repairs.shells.take_flight())
}

/// An open edge a click would take: the model it is on, its vertices, and its ends on the
/// plate.
pub(super) struct EdgeUnder {
    id: crate::scene::ObjectId,
    vertices: [u32; 2],
    ends: [core_geometry::Vec3; 2],
}

/// The open edge nearest `cursor` on the screen within reach, the nearest of those no
/// deeper than the surface under the cursor first: a ray to a rim that thin would graze
/// the faces round it. `None` past `HOVER_EDGES` edges, which are too many to look over.
pub(super) fn edge_under(
    window: &Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
) -> Option<EdgeUnder> {
    let on_screen = projector(&window.view.camera, viewport);
    let mut near: Vec<(f32, EdgeUnder)> = Vec::new();
    for object in window.doc.scene.targets() {
        let Some(open) = window.doc.repairs.found.open_edges(object) else {
            continue;
        };
        if open.ends.len() > HOVER_EDGES {
            continue;
        }
        let matrix = object.transform.to_matrix();
        for (edge, vertices) in open.ends.iter().zip(open.vertices) {
            let ends = edge.map(|end| matrix.transform_point3(end));
            let (Some(a), Some(b)) = (on_screen(ends[0]), on_screen(ends[1])) else {
                continue;
            };
            let reach = segment_distance(cursor, a, b);
            if reach <= EDGE_REACH_POINTS {
                let under = EdgeUnder {
                    id: object.id,
                    vertices: *vertices,
                    ends,
                };
                near.push((reach, under));
            }
        }
    }
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    let camera = &window.view.camera;
    let forward = (camera.target - camera.eye()).normalize_or_zero();
    let depth = |point: core_geometry::Vec3| (point - camera.eye()).dot(forward);
    let surface = super::picking::surface_under(window, viewport, cursor)
        .map(|(_, hit)| depth(hit.point) + camera.distance_mm * BEHIND_SHARE);
    let in_front = |(_, under): &(f32, EdgeUnder)| {
        let middle = (under.ends[0] + under.ends[1]) / 2.0;
        surface.is_none_or(|limit| depth(middle) <= limit)
    };
    let first = near.iter().position(in_front).unwrap_or(0);
    (!near.is_empty()).then(|| near.swap_remove(first).1)
}

/// Picks the open edge under `cursor`, as the Faces mode's Edges picking does, or says why
/// there is none.
pub(super) fn pick_edge(window: &mut Window, viewport: egui::Rect, cursor: egui::Pos2) {
    let Some(under) = edge_under(window, viewport, cursor) else {
        window.machine.status =
            crate::status::Status::Info(crate::i18n::tr!("repair-no-edge-near"));
        return;
    };
    if let Some(object) = window.doc.scene.get(under.id) {
        window.doc.repairs.hand.pick_edge(object, under.vertices);
    }
}

/// The open edge a click would take, bright over the rest, while the pointer is on the
/// viewport in the Edges picking.
pub(super) fn hovered(
    window: &Window,
    viewport: egui::Rect,
    cursor: Option<egui::Pos2>,
) -> Highlight {
    let under = cursor.and_then(|cursor| edge_under(window, viewport, cursor));
    Highlight {
        faces: Vec::new(),
        edges: under
            .map(|under| EdgeMark {
                edges: vec![under.ends],
                colour: theme::scene().fault,
            })
            .into_iter()
            .collect(),
    }
}

/// Every open edge of the models the tool works on, in `fault_rest`, to be picked among.
fn open_edges(window: &Window) -> Highlight {
    let mut edges = Vec::new();
    for object in window.doc.scene.targets() {
        if let Some(open) = window.doc.repairs.found.open_edges(object) {
            let matrix = object.transform.to_matrix();
            edges.extend(
                open.ends
                    .iter()
                    .map(|edge| edge.map(|end| matrix.transform_point3(end))),
            );
        }
    }
    Highlight {
        faces: Vec::new(),
        edges: vec![EdgeMark {
            edges,
            colour: theme::scene().fault_rest,
        }],
    }
}

/// Points from `point` to the segment from `a` to `b` on the screen.
fn segment_distance(point: egui::Pos2, a: egui::Pos2, b: egui::Pos2) -> f32 {
    let along = b - a;
    let share = ((point - a).dot(along) / along.length_sq().max(f32::EPSILON)).clamp(0.0, 1.0);
    (a + along * share).distance(point)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use core_geometry::{Transform, Vec3, cuboid, diagnose};

    use super::*;
    use crate::camera::OrbitCamera;
    use crate::scene::{ImportSummary, Imported};
    use crate::state::{Doc, Machine, Tools, View};
    use crate::workspace::{Mode, Tool};

    /// A 20 mm box missing its top, standing on the plate, picked.
    fn open_box(doc: &mut Doc) -> crate::scene::ObjectId {
        let mut mesh = cuboid(Vec3::new(50.0, 50.0, 0.0), Vec3::new(70.0, 70.0, 20.0));
        let vertices = mesh.vertices.clone();
        mesh.faces.retain(|face| {
            !face
                .iter()
                .all(|&corner| (vertices[corner as usize].z - 20.0).abs() < 1e-4)
        });
        let id = doc.scene.insert(Imported::new(
            "open.stl".to_owned(),
            Arc::new(mesh.clone()),
            Transform::default(),
            ImportSummary::made(diagnose(&mesh)),
        ));
        doc.scene.select(Some(id));
        id
    }

    #[test]
    fn a_click_on_the_rim_of_a_hole_picks_that_open_edge() {
        let mut doc = Doc::default();
        let id = open_box(&mut doc);
        let ids = [id];
        doc.repairs.found.place(&doc.scene, &ids);
        while doc.repairs.found.poll(&doc.scene) {
            std::thread::yield_now();
        }
        let (mut mode, mut tool) = (Mode::default(), Tool::Repair);
        let (mut view, mut tools, mut machine) = (
            View::framing(&doc.plate),
            Tools::default(),
            Machine::default(),
        );
        view.camera = OrbitCamera {
            target: Vec3::new(60.0, 60.0, 10.0),
            distance_mm: 80.0,
            ..OrbitCamera::default()
        };
        let mut window = Window {
            mode: &mut mode,
            tool: &mut tool,
            doc: &mut doc,
            view: &mut view,
            tools: &mut tools,
            machine: &mut machine,
        };
        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        // The middle of the rim's front edge, at y 50 and z 20.
        let camera = window.view.camera;
        let cursor = projector(&camera, viewport)(Vec3::new(60.0, 50.0, 20.0))
            .expect("in front of the camera");

        pick_edge(&mut window, viewport, cursor);
        assert_eq!(
            window
                .doc
                .repairs
                .hand
                .picked_edges(&window.doc.scene)
                .len(),
            1
        );
    }

    #[test]
    fn a_rim_behind_the_wall_is_still_picked_when_it_is_the_only_one_in_reach() {
        let mut doc = Doc::default();
        let id = open_box(&mut doc);
        doc.repairs.found.place(&doc.scene, &[id]);
        while doc.repairs.found.poll(&doc.scene) {
            std::thread::yield_now();
        }
        let (mut mode, mut tool) = (Mode::default(), Tool::Repair);
        let (mut view, mut tools, mut machine) = (
            View::framing(&doc.plate),
            Tools::default(),
            Machine::default(),
        );
        // Level with the rim from the front, so the back edge sits behind the front wall.
        view.camera = OrbitCamera {
            target: Vec3::new(60.0, 60.0, 12.0),
            distance_mm: 80.0,
            pitch_rad: 0.0,
            ..OrbitCamera::default()
        };
        let mut window = Window {
            mode: &mut mode,
            tool: &mut tool,
            doc: &mut doc,
            view: &mut view,
            tools: &mut tools,
            machine: &mut machine,
        };
        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let camera = window.view.camera;
        let behind = projector(&camera, viewport)(Vec3::new(60.0, 70.0, 20.0))
            .expect("in front of the camera");
        // A little under the back edge's image, which is on the front wall: depth only
        // orders what is in reach, so a click never comes back empty-handed for it.
        pick_edge(&mut window, viewport, behind + egui::vec2(0.0, 6.0));
        assert_eq!(
            window
                .doc
                .repairs
                .hand
                .picked_edges(&window.doc.scene)
                .len(),
            1
        );

        pick_edge(&mut window, viewport, behind + egui::vec2(0.0, 200.0));
        assert_eq!(
            window.machine.status,
            crate::status::Status::Info(crate::i18n::tr!("repair-no-edge-near")),
            "a click far from every edge says so"
        );
    }

    #[test]
    fn a_point_is_as_far_from_a_segment_as_from_its_nearest_point_on_it() {
        let (a, b) = (egui::pos2(0.0, 0.0), egui::pos2(10.0, 0.0));
        assert!((segment_distance(egui::pos2(5.0, 3.0), a, b) - 3.0).abs() < 1e-5);
        assert!(
            (segment_distance(egui::pos2(13.0, 4.0), a, b) - 5.0).abs() < 1e-5,
            "past b"
        );
        assert!(
            (segment_distance(egui::pos2(2.0, 0.0), a, a) - 2.0).abs() < 1e-5,
            "a point"
        );
    }
}
