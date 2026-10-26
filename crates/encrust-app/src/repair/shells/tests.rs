use super::*;
use crate::scene::{ImportSummary, Imported, ObjectId};
use core_geometry::{Transform, Vec3, cuboid, diagnose};

/// A 10 mm cube and a 0.5 mm one 20 mm off it, as one model.
fn cube_and_a_crumb(scene: &mut Scene) -> ObjectId {
    let mut mesh = cuboid(Vec3::ZERO, Vec3::splat(10.0));
    mesh.append(&cuboid(Vec3::splat(20.0), Vec3::splat(20.5)));
    scene.insert(Imported::new(
        "two.stl".to_owned(),
        Arc::new(mesh.clone()),
        Transform::default(),
        ImportSummary::made(diagnose(&mesh)),
    ))
}

fn listed(scene: &Scene, id: ObjectId) -> Shells {
    let mut shells = Shells::default();
    shells.list(scene.get(id).expect("inserted"));
    while shells.poll(scene) {
        std::thread::yield_now();
    }
    shells
}

#[test]
fn a_cube_and_a_crumb_list_as_two_closed_shells_in_the_order_of_their_faces() {
    let mut scene = Scene::default();
    let id = cube_and_a_crumb(&mut scene);
    let shells = listed(&scene, id);
    let rows = shells.rows(scene.get(id).expect("there")).expect("listed");
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|row| row.closed && row.faces.len() == 12));
    assert!(
        (rows[0].area_mm2 - 600.0).abs() < 1e-2,
        "got {}",
        rows[0].area_mm2
    );
}

#[test]
fn a_picked_shell_is_marked_and_flown_to_at_its_middle() {
    let mut scene = Scene::default();
    let id = cube_and_a_crumb(&mut scene);
    let mut shells = listed(&scene, id);
    let object = scene.get(id).expect("there");
    shells.toggle(object, 1);

    assert_eq!(shells.picked(object), &[1]);
    let spot = shells.take_flight().expect("the camera goes to the crumb");
    assert!(
        (spot.center - Vec3::splat(20.25)).length() < 1e-4,
        "got {}",
        spot.center
    );
    assert_eq!(shells.highlight(&scene).faces.len(), 1);
    shells.toggle(object, 1);
    assert!(shells.picked(object).is_empty() && shells.take_flight().is_none());
}

#[test]
fn shells_under_a_hundredth_of_the_largest_are_small_and_inverting_picks_the_rest() {
    let mut scene = Scene::default();
    let id = cube_and_a_crumb(&mut scene);
    let mut shells = listed(&scene, id);
    let object = scene.get(id).expect("there");
    shells.pick_small(object);
    assert_eq!(
        shells.picked(object),
        &[1],
        "the crumb is 1.5 mm², a 400th of the cube's 600"
    );
    shells.invert(object);
    assert_eq!(shells.picked(object), &[0]);
}
