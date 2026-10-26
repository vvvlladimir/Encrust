use super::*;
use crate::scene::{ImportSummary, Imported};
use core_geometry::{Transform, cuboid, diagnose};

/// `mesh` on the plate, standing 10 mm along X.
fn placed_at_ten(scene: &mut Scene, mesh: Mesh) -> ObjectId {
    scene.insert(Imported::new(
        "model.stl".to_owned(),
        Arc::new(mesh.clone()),
        Transform::from_translation(Vec3::new(10.0, 0.0, 0.0)),
        ImportSummary::made(diagnose(&mesh)),
    ))
}

/// `mesh` with the faces lying flat at height `z` taken off: a square hole there.
fn topless(mut mesh: Mesh, z: f32) -> Mesh {
    let vertices = mesh.vertices.clone();
    mesh.faces.retain(|face| {
        !face
            .iter()
            .all(|&corner| (vertices[corner as usize].z - z).abs() < 1e-6)
    });
    mesh
}

/// Two unit boxes, one at the origin and one 5 mm up the diagonal, each missing its top.
fn two_open_boxes() -> Mesh {
    let mut mesh = topless(cuboid(Vec3::ZERO, Vec3::ONE), 1.0);
    mesh.append(&topless(cuboid(Vec3::splat(5.0), Vec3::splat(6.0)), 6.0));
    mesh
}

fn settle_all(found: &mut Found, scene: &Scene) {
    while found.poll(scene) {
        std::thread::yield_now();
    }
}

/// `found` with `finding` pressed and the places of `id` worked out.
fn pressed(scene: &Scene, id: ObjectId, finding: Finding) -> Found {
    let mut found = Found::default();
    found.press(scene, finding);
    found.place(scene, &[id]);
    settle_all(&mut found, scene);
    found
}

#[test]
fn the_rim_of_a_hole_is_marked_where_the_model_stands_and_the_one_in_view_leads() {
    let mut scene = Scene::default();
    let id = placed_at_ten(&mut scene, topless(cuboid(Vec3::ZERO, Vec3::ONE), 1.0));
    let found = pressed(&scene, id, Finding::OpenEdges);

    let highlight = found.highlight(&scene);
    let [rest, focused] = highlight.edges.as_slice() else {
        panic!(
            "every rim, then the rim in view; got {}",
            highlight.edges.len()
        );
    };
    assert_eq!(
        (rest.colour, focused.colour),
        (theme::scene().fault_rest, theme::scene().fault)
    );
    assert_eq!(
        (rest.edges.len(), focused.edges.len()),
        (4, 4),
        "the one square rim"
    );
    for end in rest.edges.iter().flatten() {
        assert!(
            (end.z - 1.0).abs() < 1e-6 && (10.0..=11.0).contains(&end.x),
            "every end on the rim at z = 1, moved 10 mm along X; got {end}"
        );
    }
}

#[test]
fn a_hole_in_the_top_is_flown_to_from_above_its_middle() {
    let mut scene = Scene::default();
    let id = placed_at_ten(&mut scene, topless(cuboid(Vec3::ZERO, Vec3::ONE), 1.0));
    let mut found = pressed(&scene, id, Finding::OpenEdges);

    let spot = found
        .take_flight(&scene)
        .expect("the camera is to go to the hole");
    assert!(
        (spot.center - Vec3::new(10.5, 0.5, 1.0)).length() < 1e-5,
        "got {}",
        spot.center
    );
    assert!(
        (spot.normal - Vec3::Z).length() < 1e-5,
        "seen from above; got {}",
        spot.normal
    );
    assert!((spot.radius_mm - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-5);
    assert_eq!(found.take_flight(&scene), None, "once per press");
}

#[test]
fn each_press_goes_to_the_next_hole_and_the_one_after_the_last_puts_the_marks_away() {
    let mut scene = Scene::default();
    let id = placed_at_ten(&mut scene, two_open_boxes());
    let mut found = pressed(&scene, id, Finding::OpenEdges);
    let first = found.take_flight(&scene).expect("the first hole");

    found.press(&scene, Finding::OpenEdges);
    let second = found.take_flight(&scene).expect("the second hole");
    assert!(
        (first.center.z - second.center.z).abs() > 4.0,
        "one hole at 1 mm up, the other at 6; got {} and {}",
        first.center,
        second.center
    );

    found.press(&scene, Finding::OpenEdges);
    assert_eq!(found.shown(), None);
    assert!(found.highlight(&scene).edges.is_empty() && found.take_flight(&scene).is_none());
}

#[test]
fn faces_drawn_twice_are_marked_as_a_patch_and_the_piece_in_view_over_it() {
    let mut scene = Scene::default();
    let mut mesh = cuboid(Vec3::ZERO, Vec3::ONE);
    let first = mesh.faces[0];
    mesh.faces.push(first);
    let doubled = diagnose(&mesh).duplicate_faces;
    let id = placed_at_ten(&mut scene, mesh);
    let found = pressed(&scene, id, Finding::DoubledFaces);

    let highlight = found.highlight(&scene);
    let [all, piece] = highlight.faces.as_slice() else {
        panic!(
            "the patch and the piece in view, got {}",
            highlight.faces.len()
        );
    };
    assert_eq!(all.mesh.faces.len(), doubled);
    assert_eq!(
        (all.colour, piece.colour),
        (theme::scene().fault_rest, theme::scene().fault)
    );
    assert_eq!(piece.transform.translation, Vec3::new(10.0, 0.0, 0.0));
}

#[test]
fn nothing_is_marked_until_a_finding_is_pressed() {
    let mut scene = Scene::default();
    let id = placed_at_ten(&mut scene, topless(cuboid(Vec3::ZERO, Vec3::ONE), 1.0));
    let mut found = Found::default();
    found.place(&scene, &[id]);
    settle_all(&mut found, &scene);

    let highlight = found.highlight(&scene);
    assert!(highlight.edges.is_empty() && highlight.faces.is_empty());
    assert_eq!(found.take_flight(&scene), None);
}

#[test]
fn two_crossing_cubes_mark_their_crossing_faces_until_the_mesh_changes() {
    let mut scene = Scene::default();
    // Pushed in off the diagonals, so no edge of one lies along a face's own seam.
    let shift = Vec3::new(0.5, 0.3, 0.4);
    let mut crossing = cuboid(Vec3::ZERO, Vec3::ONE);
    crossing.append(&cuboid(shift, shift + Vec3::ONE));
    let id = placed_at_ten(&mut scene, crossing);
    let mut found = Found::default();
    found.press(&scene, Finding::CrossingFaces);
    let object = scene.get(id).expect("inserted");
    assert_eq!(found.crossings_of(object), None, "nothing counted yet");

    found.count_crossings(&scene, &[id]);
    settle_all(&mut found, &scene);
    let object = scene.get(id).expect("still there");
    let counted = found.crossings_of(object).flatten();
    assert!(counted.is_some_and(|pairs| pairs > 0), "got {counted:?}");
    assert!(
        !found.highlight(&scene).faces.is_empty(),
        "the crossing faces are marked"
    );
    assert!(
        found.take_flight(&scene).is_some(),
        "and the camera goes to them"
    );

    let reshaped = crate::scene::Shaped::of(Arc::new(cuboid(Vec3::ZERO, Vec3::ONE)));
    scene.get_mut(id).expect("still there").reshape(reshaped);
    settle_all(&mut found, &scene);
    let object = scene.get(id).expect("still there");
    assert_eq!(
        found.crossings_of(object),
        None,
        "the count was of the old mesh"
    );
    assert!(
        found.highlight(&scene).faces.is_empty(),
        "and so were the marks"
    );
}

#[test]
fn a_finding_with_no_place_on_the_models_has_no_flight_and_a_second_press_puts_it_away() {
    let mut scene = Scene::default();
    let id = placed_at_ten(&mut scene, cuboid(Vec3::ZERO, Vec3::ONE));
    let mut found = pressed(&scene, id, Finding::OpenEdges);

    assert_eq!(found.take_flight(&scene), None);
    found.press(&scene, Finding::OpenEdges);
    assert_eq!(found.shown(), None);
}

#[test]
fn no_flight_is_taken_while_a_target_is_still_worked_out() {
    let mut scene = Scene::default();
    let first = placed_at_ten(&mut scene, topless(cuboid(Vec3::ZERO, Vec3::ONE), 1.0));
    let mut found = pressed(&scene, first, Finding::OpenEdges);
    let second = placed_at_ten(&mut scene, topless(cuboid(Vec3::ZERO, Vec3::ONE), 1.0));
    found.place(&scene, &[second]);

    assert_eq!(
        found.take_flight(&scene),
        None,
        "the second model's holes come before the count is known"
    );
    settle_all(&mut found, &scene);
    assert!(
        found.take_flight(&scene).is_some(),
        "both are worked out now"
    );
}
