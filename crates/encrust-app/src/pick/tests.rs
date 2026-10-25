use super::*;
use crate::scene::ImportSummary;
use core_geometry::{Mesh, Orientation, Transform, diagnose};
use std::sync::Arc;

const VIEWPORT: egui::Rect = egui::Rect {
    min: egui::Pos2::new(0.0, 0.0),
    max: egui::Pos2::new(800.0, 600.0),
};

fn center() -> egui::Pos2 {
    VIEWPORT.center()
}

/// Axis-aligned cube spanning 0..1 on every axis.
fn unit_cube() -> Mesh {
    core_geometry::cuboid(Vec3::ZERO, Vec3::ONE)
}

fn summary() -> ImportSummary {
    ImportSummary {
        vertices_merged: 0,
        faces_removed: 0,
        orientation: Orientation {
            flipped_faces: 0,
            inverted_shells: 0,
            orientable: true,
        },
        diagnostics: diagnose(&unit_cube()),
    }
}

fn add(scene: &mut Scene, name: &str, at: Vec3) -> ObjectId {
    scene.insert(crate::scene::Imported::new(
        name.to_owned(),
        Arc::new(unit_cube()),
        Transform::from_translation(at),
        summary(),
    ))
}

/// A camera whose target is the middle of the cube placed at `at`.
fn looking_at(at: Vec3) -> OrbitCamera {
    OrbitCamera {
        target: at + Vec3::splat(0.5),
        distance_mm: 20.0,
        ..OrbitCamera::default()
    }
}

#[test]
fn the_centre_of_the_viewport_hits_what_the_camera_looks_at() {
    let mut scene = Scene::default();
    let id = add(&mut scene, "cube", Vec3::ZERO);
    let camera = looking_at(Vec3::ZERO);
    assert_eq!(
        pick(&scene, &camera, VIEWPORT, center(), Shown::default()),
        Some(id)
    );
}

#[test]
fn the_nearer_of_two_stacked_objects_wins() {
    let mut scene = Scene::default();
    let far = add(&mut scene, "far", Vec3::ZERO);
    let camera = looking_at(Vec3::ZERO);

    // A second cube centred 5 mm from the target towards the eye, so it sits
    // squarely in front of the first one.
    let towards_eye = (camera.eye() - camera.target).normalize() * 5.0;
    let near = add(&mut scene, "near", towards_eye);

    let picked = pick(&scene, &camera, VIEWPORT, center(), Shown::default());
    assert_eq!(picked, Some(near));
    assert_ne!(picked, Some(far));
}

#[test]
fn a_corner_of_the_viewport_hits_nothing() {
    let mut scene = Scene::default();
    add(&mut scene, "cube", Vec3::ZERO);
    let camera = looking_at(Vec3::ZERO);
    assert_eq!(
        pick(&scene, &camera, VIEWPORT, VIEWPORT.min, Shown::default()),
        None
    );
}

#[test]
fn a_hidden_object_cannot_be_picked() {
    let mut scene = Scene::default();
    add(&mut scene, "cube", Vec3::ZERO);
    scene.objects_mut()[0].visible = false;
    let camera = looking_at(Vec3::ZERO);
    assert_eq!(
        pick(&scene, &camera, VIEWPORT, center(), Shown::default()),
        None
    );
}

#[test]
fn a_flattened_object_cannot_be_picked() {
    let mut scene = Scene::default();
    add(&mut scene, "cube", Vec3::ZERO);
    scene.objects_mut()[0].transform.scale = Vec3::new(1.0, 1.0, 0.0);
    let camera = looking_at(Vec3::ZERO);
    assert_eq!(
        pick(&scene, &camera, VIEWPORT, center(), Shown::default()),
        None
    );
}

#[test]
fn a_viewport_with_no_area_produces_no_ray() {
    let camera = OrbitCamera::default();
    let empty = egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::ZERO);
    assert!(ray_through(&camera, empty, egui::Pos2::ZERO).is_none());
}

#[test]
fn the_ray_through_the_centre_runs_down_the_view_axis() {
    let camera = looking_at(Vec3::ZERO);
    let ray = ray_through(&camera, VIEWPORT, center()).expect("the viewport has area");
    let axis = (camera.target - camera.eye()).normalize();
    assert!(
        ray.direction.abs_diff_eq(axis, 1e-4),
        "expected the view axis {axis}, got {}",
        ray.direction
    );
}

/// Straight down on the plate from 20 mm over the point (0.5, 0.5).
fn from_above() -> OrbitCamera {
    OrbitCamera {
        target: Vec3::new(0.5, 0.5, 0.0),
        distance_mm: 20.0,
        pitch_rad: std::f32::consts::FRAC_PI_2 - 1e-3,
        ..OrbitCamera::default()
    }
}

#[test]
fn a_face_the_section_cut_away_is_passed_through_to_the_one_under_it() {
    // A cube on the floor and a second one standing over it from 2 to 3 mm.
    let mut scene = Scene::default();
    let floor = add(&mut scene, "floor", Vec3::ZERO);
    add(&mut scene, "over", Vec3::new(0.0, 0.0, 2.0));
    let cut = Shown {
        top_mm: Some(1.5),
        ..Shown::default()
    };

    let (id, hit) = pick_surface(&scene, &from_above(), VIEWPORT, center(), cut)
        .expect("the lower cube's lid is drawn under the cut");
    assert_eq!(id, floor);
    assert!(
        (hit.point.z - 1.0).abs() < 1e-3,
        "the lid of the lower cube is at 1 mm, got {}",
        hit.point
    );
}

#[test]
fn the_cap_across_a_cut_is_material_but_no_surface() {
    let mut scene = Scene::default();
    let cube = add(&mut scene, "cube", Vec3::ZERO);
    let cut = Shown {
        top_mm: Some(0.5),
        ..Shown::default()
    };

    assert!(
        pick_surface(&scene, &from_above(), VIEWPORT, center(), cut).is_none(),
        "the cap has no face of the model to put anything on"
    );
    let (id, hit) = pick_material(&scene, &from_above(), VIEWPORT, center(), cut)
        .expect("the cap is the cube's material");
    assert_eq!(id, cube);
    assert!(
        (hit.point.z - 0.5).abs() < 1e-3 && hit.normal.z > 0.99,
        "the cap lies on the cut and faces up out of what is left, got {} {}",
        hit.point,
        hit.normal
    );
    assert_eq!(
        pick(&scene, &from_above(), VIEWPORT, center(), cut),
        Some(cube),
        "a click on the cap selects the model it cuts"
    );
}

#[test]
fn the_cap_is_met_when_the_face_out_of_the_material_lies_under_the_floor_cut() {
    let mut scene = Scene::default();
    let cube = add(&mut scene, "cube", Vec3::ZERO);
    let cut = Shown {
        floor_mm: Some(0.2),
        top_mm: Some(0.5),
    };
    let (id, hit) = pick_material(&scene, &from_above(), VIEWPORT, center(), cut)
        .expect("the cap at 0.5 mm is drawn over the cube's material");
    assert_eq!(id, cube);
    assert!(
        (hit.point.z - 0.5).abs() < 1e-3,
        "the cap lies on the top cut at 0.5 mm, got {}",
        hit.point
    );
}

#[test]
fn the_cap_hides_a_face_of_another_model_inside_the_material() {
    // The upper cube stands from 1 to 2 mm; the lower one reaches into it up to 1.4 mm.
    let mut scene = Scene::default();
    let upper = add(&mut scene, "upper", Vec3::new(0.0, 0.0, 1.0));
    add(&mut scene, "lower", Vec3::new(0.0, 0.0, 0.4));
    let cut = Shown {
        top_mm: Some(1.8),
        ..Shown::default()
    };
    assert!(
        pick_surface(&scene, &from_above(), VIEWPORT, center(), cut).is_none(),
        "the lower cube's lid at 1.4 mm is under the upper cube's cap"
    );
    assert_eq!(
        pick(&scene, &from_above(), VIEWPORT, center(), cut),
        Some(upper)
    );
}

#[test]
fn nothing_under_the_floor_cut_can_be_pointed_at() {
    let mut scene = Scene::default();
    add(&mut scene, "cube", Vec3::ZERO);
    let cut = Shown {
        floor_mm: Some(2.0),
        ..Shown::default()
    };
    assert_eq!(pick(&scene, &from_above(), VIEWPORT, center(), cut), None);
}

#[test]
fn a_point_behind_a_model_is_hidden_and_one_beside_it_is_not() {
    let mut scene = Scene::default();
    add(&mut scene, "cube", Vec3::ZERO);
    let eye = Vec3::new(0.5, 0.5, 10.0);
    assert!(
        occluded(&scene, eye, Vec3::new(0.5, 0.5, -5.0)),
        "the unit cube stands between the eye and a point under it"
    );
    assert!(!occluded(&scene, eye, Vec3::new(5.0, 0.5, -5.0)));
    assert!(
        !occluded(&scene, eye, Vec3::new(0.5, 0.5, 1.0)),
        "a point on the cube's own top face is not hidden by it"
    );
}

/// The unit cube hollowed to a cavity spanning 0.2..0.8, as a shell keeps it: the cube
/// with that box appended wound inward.
fn hollowed(scene: &mut Scene, id: ObjectId) {
    let mut shell = unit_cube();
    let inner = unit_cube();
    let offset = shell.vertices.len() as u32;
    shell
        .vertices
        .extend(inner.vertices.iter().map(|at| *at * 0.6 + Vec3::splat(0.2)));
    shell.faces.extend(
        inner
            .faces
            .iter()
            .map(|[a, b, c]| [a + offset, c + offset, b + offset]),
    );
    let object = scene.get_mut(id).expect("the cube is on the plate");
    let settings = object
        .hollow
        .asking(&core_volume::HollowSettings::for_wall(0.2));
    object.hollow.take(core_volume::Shell {
        mesh: Arc::new(shell),
        cavity: 12..24,
        cavity_mm3: 0.216,
        voxel_mm: 0.05,
        coarsened: false,
        shape: object.transform.unturned(),
        settings,
    });
    object.keep_support_surface();
}

#[test]
fn a_cut_through_a_cavity_is_seen_into_down_to_its_floor() {
    let mut scene = Scene::default();
    let cube = add(&mut scene, "cube", Vec3::ZERO);
    hollowed(&mut scene, cube);
    let cut = Shown {
        top_mm: Some(0.5),
        ..Shown::default()
    };
    let camera = from_above();
    let down = ray_through(&camera, VIEWPORT, center()).expect("a ray down the middle");
    let (near, wall) = seen_along(&scene, &down, cut).expect("the cut lets the ray in");

    assert!(
        (down.at(wall).z - 0.2).abs() < 1e-3,
        "the cavity's floor at 0.2 mm hides what is under it, not the cut at {}",
        down.at(near).z
    );
}

#[test]
fn a_cut_through_the_wall_is_capped() {
    let mut scene = Scene::default();
    let cube = add(&mut scene, "cube", Vec3::ZERO);
    hollowed(&mut scene, cube);
    let cut = Shown {
        top_mm: Some(0.5),
        ..Shown::default()
    };
    // Down through the wall, a tenth in from the cube's edge.
    let ray = Ray::new(Vec3::new(0.1, 0.5, 20.0), Vec3::NEG_Z);
    let (near, wall) = seen_along(&scene, &ray, cut).expect("the cut lets the ray in");
    assert!(
        (near - wall).abs() < 1e-4,
        "the cap at the cut hides the wall's inside"
    );
}

/// A 10 mm block with a 3 mm hole 5 mm down into the middle of its top, taken out exactly,
/// and the printed surface indexed, as the window has it a moment after the click.
fn drilled_block(scene: &mut Scene) -> ObjectId {
    let mesh = core_geometry::cuboid(Vec3::ZERO, Vec3::splat(10.0));
    let id = scene.insert(crate::scene::Imported::new(
        "block".to_owned(),
        Arc::new(mesh.clone()),
        Transform::default(),
        summary(),
    ));
    let object = scene.get_mut(id).expect("the block is on the plate");
    let bvh = core_geometry::Bvh::build(&mesh);
    let size = core_volume::HoleSize::round(3.0, 5.0, 1.0);
    let top = Vec3::new(5.0, 5.0, 10.0);
    object
        .hollow
        .add_drain(&mesh, &bvh, top, Vec3::Z, size, Transform::default());
    let cuts = Arc::clone(object.hollow.cut_bodies().expect("a hole is a cut"));
    let source = Arc::clone(object.uncut());
    let drilled = core_volume::Drilled::of(&source, None, &cuts).expect("a block drills");
    object.hollow.keep_drilled(&source, &cuts, Some(drilled));
    object.keep_support_surface();
    id
}

fn straight_down_on(point: Vec3) -> OrbitCamera {
    OrbitCamera {
        target: point,
        distance_mm: 40.0,
        pitch_rad: std::f32::consts::FRAC_PI_2 - 1e-4,
        ..OrbitCamera::default()
    }
}

#[test]
fn pointing_into_a_drilled_hole_meets_its_floor_not_the_mouth_it_took() {
    let mut scene = Scene::default();
    let id = drilled_block(&mut scene);
    let camera = straight_down_on(Vec3::new(5.0, 5.0, 5.0));
    let (hit_id, printed, hit) =
        pick_printed(&scene, &camera, VIEWPORT, center(), Shown::default()).expect("the hole");
    assert_eq!((hit_id, printed), (id, Printed::Model));
    assert!(
        (hit.point.z - 5.0).abs() < 1e-3,
        "the floor stands 5 mm down at z = 5, got {}",
        hit.point.z
    );
}

#[test]
fn a_raft_beside_its_model_is_pointed_at_as_its_supports() {
    let mut scene = Scene::default();
    let id = add(&mut scene, "cube", Vec3::new(0.0, 0.0, 2.0));
    let raft = core_supports::shaded(vec![core_geometry::cuboid(
        Vec3::new(-5.0, -5.0, 0.0),
        Vec3::new(6.0, 6.0, 1.0),
    )]);
    let bvhs = vec![Arc::new(core_geometry::Bvh::build(&raft[0].mesh))];
    let object = scene.get_mut(id).expect("the cube");
    // Built for no groups of its own, so it stands for whatever the tool built.
    let from = crate::drilling::BuiltFrom::of(crate::drilling::SupportsUnder {
        groups: &[],
        cuts: None,
        transform: object.transform,
    });
    object.printed_supports.keep(from, raft, bvhs);

    let beside = straight_down_on(Vec3::new(-3.0, -3.0, 1.0));
    let (hit_id, printed, hit) =
        pick_printed(&scene, &beside, VIEWPORT, center(), Shown::default()).expect("the raft");
    assert_eq!((hit_id, printed), (id, Printed::Supports(0)));
    assert!(
        (hit.point.z - 1.0).abs() < 1e-3,
        "on the raft's top, got {hit:?}"
    );

    let over = straight_down_on(Vec3::new(0.5, 0.5, 3.0));
    let (_, printed, _) =
        pick_printed(&scene, &over, VIEWPORT, center(), Shown::default()).expect("the cube");
    assert_eq!(
        printed,
        Printed::Model,
        "the model in front of the raft is met first"
    );
}
