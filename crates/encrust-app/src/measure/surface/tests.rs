use std::time::{Duration, Instant};

use super::*;
use crate::measure::Snapped;
use crate::measure::tests::add_cube;

/// Polls `walking` until it settles, failing past a few seconds.
fn settle(walking: &mut Walking, measure: &mut Measure, scene: &Scene) {
    let mut edges = EdgeCache::default();
    let started = Instant::now();
    while walking.poll(measure, &mut edges, scene) {
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the walk never ended"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// Picks the point `end` of the unit cube on model `id`, scaled where it stands.
fn pick_at(measure: &mut Measure, scene: &Scene, id: ObjectId, end: Vec3) {
    let object = scene.get(id).expect("the cube is in the scene");
    let point = object.transform.to_matrix().transform_point3(end);
    measure.pick(point, Snapped::Free, Anchor::of(object, point));
}

/// Measures along the surface of a 10 mm cube from the middle of its top to the middle of a
/// side: 5 mm to the edge and 5 mm down.
fn top_to_side() -> (Scene, ObjectId, Measure) {
    let mut scene = Scene::default();
    let id = add_cube(&mut scene, 10.0);
    let mut measure = Measure {
        along_surface: true,
        ..Measure::default()
    };
    pick_at(&mut measure, &scene, id, Vec3::new(0.5, 0.5, 1.0));
    pick_at(&mut measure, &scene, id, Vec3::new(0.5, 0.0, 0.5));
    settle(&mut Walking::default(), &mut measure, &scene);
    (scene, id, measure)
}

#[test]
fn a_walk_from_the_top_of_a_cube_to_a_side_runs_over_the_edge() {
    let (_, _, measure) = top_to_side();
    let length_mm = measure.surface_path().expect("one closed cube").length_mm();
    assert!(
        (length_mm - 10.0).abs() < 1e-3,
        "5 mm to the edge and 5 mm down; got {length_mm}"
    );
}

#[test]
fn points_on_two_models_have_no_walk() {
    let mut scene = Scene::default();
    let (first, second) = (add_cube(&mut scene, 10.0), add_cube(&mut scene, 10.0));
    let mut measure = Measure {
        along_surface: true,
        ..Measure::default()
    };
    pick_at(&mut measure, &scene, first, Vec3::new(0.5, 0.5, 1.0));
    pick_at(&mut measure, &scene, second, Vec3::new(0.5, 0.5, 1.0));
    settle(&mut Walking::default(), &mut measure, &scene);
    assert_eq!(measure.walk(), Some(&Walk::Apart));
}

#[test]
fn a_new_pick_forgets_the_walk() {
    let (_, _, mut measure) = top_to_side();
    measure.pick(Vec3::ZERO, Snapped::Free, None);
    assert_eq!(measure.walk(), None);
}

#[test]
fn a_walk_is_dropped_once_its_model_moves() {
    let (mut scene, id, mut measure) = top_to_side();
    scene
        .get_mut(id)
        .expect("the cube is in the scene")
        .transform
        .translation += Vec3::X;
    settle(&mut Walking::default(), &mut measure, &scene);
    assert_eq!(measure.walk(), Some(&Walk::Apart));
}

#[test]
fn a_cancelled_walk_says_so_and_is_not_started_again() {
    let mut scene = Scene::default();
    let id = add_cube(&mut scene, 10.0);
    let mut measure = Measure {
        along_surface: true,
        ..Measure::default()
    };
    pick_at(&mut measure, &scene, id, Vec3::new(0.5, 0.5, 1.0));
    pick_at(&mut measure, &scene, id, Vec3::new(0.5, 0.5, 0.0));
    let (mut walking, mut edges) = (Walking::default(), EdgeCache::default());
    assert!(
        walking.poll(&mut measure, &mut edges, &scene),
        "the walk starts"
    );
    walking.cancel(&mut measure);
    assert_eq!(measure.walk(), Some(&Walk::Stopped));
    assert!(
        !walking.poll(&mut measure, &mut edges, &scene),
        "nothing runs again"
    );
    assert_eq!(walking.notice(), None);
}
