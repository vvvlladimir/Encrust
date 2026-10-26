use super::*;
use crate::scene::{ImportSummary, Imported};
use core_geometry::{Transform, cuboid, diagnose, faults};

fn insert(scene: &mut Scene, mesh: Mesh) -> ObjectId {
    scene.insert(Imported::new(
        "model.stl".to_owned(),
        Arc::new(mesh.clone()),
        Transform::default(),
        ImportSummary::made(diagnose(&mesh)),
    ))
}

/// The unit cube with the two triangles of its top taken off: one square hole.
fn open_box() -> Mesh {
    let mut mesh = cuboid(Vec3::ZERO, Vec3::ONE);
    let vertices = mesh.vertices.clone();
    mesh.faces
        .retain(|face| face.iter().any(|&corner| vertices[corner as usize].z < 1.0));
    mesh
}

fn settle(hand: &mut Hand, scene: &mut Scene) -> Status {
    let mut status = Status::default();
    while hand.poll(scene, &mut status) {
        std::thread::yield_now();
    }
    status
}

#[test]
fn a_brush_on_a_corner_picks_the_faces_it_reaches_and_alt_lets_them_go() {
    let mut scene = Scene::default();
    let id = insert(&mut scene, cuboid(Vec3::ZERO, Vec3::splat(10.0)));
    let mut hand = Hand::default();
    let object = scene.get(id).expect("inserted");
    hand.brush(object, Vec3::ZERO, 1.0, true);
    let (_, picked) = hand.picked_faces(&scene).expect("the corner's faces");
    assert!(
        (3..=6).contains(&picked),
        "three sides meet at a corner; got {picked}"
    );

    hand.brush(scene.get(id).expect("there"), Vec3::ZERO, 1.0, false);
    assert!(hand.picked_faces(&scene).is_none());
}

#[test]
fn a_whole_side_is_picked_from_one_face_and_flipped_by_hand() {
    let mut scene = Scene::default();
    let id = insert(&mut scene, cuboid(Vec3::ZERO, Vec3::ONE));
    let mut hand = Hand::default();
    hand.flood(scene.get(id).expect("inserted"), 0, 10.0, true);
    assert_eq!(
        hand.picked_faces(&scene).map(|(_, count)| count),
        Some(2),
        "one square side"
    );

    assert!(hand.start(&scene, HandEdit::Flip, &mut Status::default()));
    let status = settle(&mut hand, &mut scene);
    assert_eq!(
        status,
        Status::Info(tr!("edit-flipped", name = "model.stl", count = 2))
    );
    let mut landed = (*scene.get(id).expect("there").mesh).clone();
    assert_eq!(core_geometry::orient_outward(&mut landed).flipped_faces, 2);
    assert!(
        hand.picked_faces(&scene).is_none(),
        "the picks went with the change"
    );
}

#[test]
fn two_picked_edges_across_a_hole_are_bridged_shut() {
    let mut scene = Scene::default();
    let mesh = open_box();
    let open = faults(&mesh).open_edges;
    let across = *open[1..]
        .iter()
        .find(|edge| !edge.iter().any(|corner| open[0].contains(corner)))
        .expect("a square rim has an edge across from each");
    let id = insert(&mut scene, mesh);
    let mut hand = Hand::default();
    let object = scene.get(id).expect("inserted");
    hand.pick_edge(object, open[0]);
    hand.pick_edge(object, across);
    assert_eq!(hand.picked_edges(&scene).len(), 2);

    assert!(hand.start(&scene, HandEdit::Bridge, &mut Status::default()));
    settle(&mut hand, &mut scene);
    assert!(
        scene.get(id).expect("there").summary.is_sound(),
        "the hole is shut"
    );
}

#[test]
fn a_third_edge_lets_the_first_go_and_a_second_click_on_one_lets_it_go() {
    let mut scene = Scene::default();
    let mesh = open_box();
    let open = faults(&mesh).open_edges;
    let id = insert(&mut scene, mesh);
    let object = scene.get(id).expect("inserted");
    let mut hand = Hand::default();
    for edge in &open[..3] {
        hand.pick_edge(object, *edge);
    }
    assert_eq!(hand.picked_edges(&scene), &open[1..3]);
    hand.pick_edge(object, [open[1][1], open[1][0]]);
    assert_eq!(
        hand.picked_edges(&scene),
        &open[2..3],
        "either way round is the same edge"
    );
}

#[test]
fn a_change_whose_model_was_replaced_meanwhile_is_dropped_and_said() {
    let mut scene = Scene::default();
    let id = insert(&mut scene, cuboid(Vec3::ZERO, Vec3::ONE));
    let mut hand = Hand::default();
    hand.flood(scene.get(id).expect("inserted"), 0, 10.0, true);
    hand.start(&scene, HandEdit::Remove, &mut Status::default());
    let other = crate::scene::Shaped::of(Arc::new(cuboid(Vec3::ZERO, Vec3::splat(2.0))));
    scene.get_mut(id).expect("there").reshape(other);

    let status = settle(&mut hand, &mut scene);
    assert_eq!(
        status,
        Status::Error(tr!("job-model-changed", name = "model.stl"))
    );
    assert!(
        scene.get(id).expect("there").summary.is_sound(),
        "the new mesh stands"
    );
}
