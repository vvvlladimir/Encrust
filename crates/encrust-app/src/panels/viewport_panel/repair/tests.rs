use std::sync::Arc;

use core_geometry::{Mesh, Transform, Vec3, cuboid, diagnose};

use super::*;
use crate::camera::OrbitCamera;
use crate::scene::{ImportSummary, Imported};
use crate::state::{Doc, Machine, Tools, View};
use crate::status::Status;
use crate::workspace::{Mode, Tool};

const VIEWPORT: egui::Rect = egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(800.0, 600.0));

/// A 20 mm box standing on the plate.
fn box_mesh() -> Mesh {
    cuboid(Vec3::new(50.0, 50.0, 0.0), Vec3::new(70.0, 70.0, 20.0))
}

/// The same box missing its top.
fn open_box() -> Mesh {
    let mut mesh = box_mesh();
    let vertices = mesh.vertices.clone();
    mesh.faces.retain(|face| {
        !face
            .iter()
            .all(|&corner| (vertices[corner as usize].z - 20.0).abs() < 1e-4)
    });
    mesh
}

/// `mesh` on the plate, picked and its open edges found, in the Repair tool seen by
/// `camera`.
fn in_repair(mesh: Mesh, camera: OrbitCamera, then: impl FnOnce(&mut Window)) {
    let mut doc = Doc::default();
    let id = doc.scene.insert(Imported::new(
        "model.stl".to_owned(),
        Arc::new(mesh.clone()),
        Transform::default(),
        ImportSummary::made(diagnose(&mesh)),
    ));
    doc.scene.select(Some(id));
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
    view.camera = camera;
    then(&mut Window {
        mode: &mut mode,
        tool: &mut tool,
        doc: &mut doc,
        view: &mut view,
        tools: &mut tools,
        machine: &mut machine,
    });
}

/// The open box and loose triangles off to its side enough for `HOVER_EDGES` open edges.
fn open_box_and_crumbs() -> Mesh {
    let mut mesh = open_box();
    let crumb = [0.0, 1.0].map(|x| Vec3::new(150.0 + x, 150.0, 0.0));
    for _ in 0..HOVER_EDGES / 3 + 1 {
        let first = mesh.vertices.len() as u32;
        mesh.vertices
            .extend([crumb[0], crumb[1], Vec3::new(150.0, 151.0, 0.0)]);
        mesh.faces.push([first, first + 1, first + 2]);
    }
    mesh
}

fn picked(window: &Window) -> usize {
    window
        .doc
        .repairs
        .hand
        .picked_edges(&window.doc.scene)
        .len()
}

#[test]
fn a_click_on_the_rim_of_a_hole_picks_that_open_edge() {
    let camera = OrbitCamera {
        target: Vec3::new(60.0, 60.0, 10.0),
        distance_mm: 80.0,
        ..OrbitCamera::default()
    };
    in_repair(open_box(), camera, |window| {
        // The middle of the rim's front edge, at y 50 and z 20.
        let cursor = projector(&camera, VIEWPORT)(Vec3::new(60.0, 50.0, 20.0))
            .expect("in front of the camera");
        pick_edge(&egui::Context::default(), window, VIEWPORT, cursor);
        assert_eq!(picked(window), 1);
    });
}

#[test]
fn a_click_picks_a_rim_on_a_model_of_too_many_open_edges_to_light_on_hover() {
    let camera = OrbitCamera {
        target: Vec3::new(60.0, 60.0, 10.0),
        distance_mm: 80.0,
        ..OrbitCamera::default()
    };
    in_repair(open_box_and_crumbs(), camera, |window| {
        let cursor = projector(&camera, VIEWPORT)(Vec3::new(60.0, 50.0, 20.0))
            .expect("in front of the camera");
        assert!(hovered(window, VIEWPORT, Some(cursor)).edges.is_empty());
        pick_edge(&egui::Context::default(), window, VIEWPORT, cursor);
        assert_eq!(picked(window), 1, "a click looks over every open edge");
    });
}

#[test]
fn a_rim_behind_the_wall_is_still_picked_when_it_is_the_only_one_in_reach() {
    // Level with the rim from the front, so the back edge sits behind the front wall.
    let camera = OrbitCamera {
        target: Vec3::new(60.0, 60.0, 12.0),
        distance_mm: 80.0,
        pitch_rad: 0.0,
        ..OrbitCamera::default()
    };
    in_repair(open_box(), camera, |window| {
        let ctx = egui::Context::default();
        let behind = projector(&camera, VIEWPORT)(Vec3::new(60.0, 70.0, 20.0))
            .expect("in front of the camera");
        // A little under the back edge's image, which is on the front wall: depth only
        // orders what is in reach, so a click never comes back empty-handed for it.
        pick_edge(&ctx, window, VIEWPORT, behind + egui::vec2(0.0, 6.0));
        assert_eq!(picked(window), 1);

        pick_edge(&ctx, window, VIEWPORT, behind + egui::vec2(0.0, 200.0));
        assert_eq!(
            window.machine.status,
            Status::Info(crate::i18n::tr!("repair-no-edge-near")),
            "a click far from every edge says so"
        );
    });
}

#[test]
fn a_click_on_a_closed_model_says_it_has_no_open_edges() {
    in_repair(box_mesh(), OrbitCamera::default(), |window| {
        pick_edge(
            &egui::Context::default(),
            window,
            VIEWPORT,
            VIEWPORT.center(),
        );
        assert_eq!(
            window.machine.status,
            Status::Info(crate::i18n::tr!("repair-no-open-edges")),
            "a closed model has no rim to click near"
        );
    });
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
