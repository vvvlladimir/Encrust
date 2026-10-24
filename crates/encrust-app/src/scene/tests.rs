use super::import_summary::ImportSummary;
use super::object::SceneObject;
use super::*;
use core_geometry::{Mesh, Orientation, Quat, Transform, Vec2, Vec3};
use std::f32::consts::FRAC_PI_2;
use std::sync::Arc;

fn summary() -> ImportSummary {
    ImportSummary {
        vertices_merged: 0,
        faces_removed: 0,
        orientation: Orientation {
            flipped_faces: 0,
            inverted_shells: 0,
            orientable: true,
        },
        diagnostics: core_geometry::diagnose(&unit_cube()),
    }
}

/// Axis-aligned cube spanning 0..1 on every axis, twelve triangles.
fn unit_cube() -> Mesh {
    core_geometry::cuboid(Vec3::ZERO, Vec3::ONE)
}

fn scene_with_cube() -> (Scene, ObjectId) {
    let mut scene = Scene::default();
    let id = scene.insert(crate::scene::Imported::new(
        "cube".to_owned(),
        Arc::new(unit_cube()),
        Transform::default(),
        summary(),
    ));
    (scene, id)
}

#[test]
fn the_handles_stand_on_the_centre_of_mass_not_the_models_origin() {
    let (mut scene, id) = scene_with_cube();
    let object = scene.get_mut(id).expect("just added");
    object.transform.translation = Vec3::new(10.0, 20.0, 0.0);

    let pivot = object.pivot();
    // The cube spans 0..1 in its own space, so its mass sits half a millimetre in.
    assert!(
        pivot
            .translation
            .abs_diff_eq(Vec3::new(10.5, 20.5, 0.5), 1e-5),
        "expected the middle of the placed cube, got {:?}",
        pivot.translation
    );
}

/// The six points of an octahedron of radius one: its own box is a cube of side two.
fn octahedron() -> Mesh {
    let vertices = vec![Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z];
    let faces = vec![
        [0, 2, 4],
        [2, 1, 4],
        [1, 3, 4],
        [3, 0, 4],
        [2, 0, 5],
        [1, 2, 5],
        [3, 1, 5],
        [0, 3, 5],
    ];
    Mesh::new(vertices, faces)
}

#[test]
fn a_turned_model_is_bounded_by_its_vertices_not_by_its_turned_box() {
    let mut scene = Scene::default();
    let id = scene.insert(crate::scene::Imported::new(
        "octahedron".to_owned(),
        Arc::new(octahedron()),
        Transform {
            rotation: Quat::from_rotation_z(std::f32::consts::FRAC_PI_4),
            ..Transform::default()
        },
        summary(),
    ));
    let bounds = scene.get(id).and_then(SceneObject::world_bounds);
    let width = bounds.map(|bounds| bounds.maxs.x - bounds.mins.x);
    // (1, 0) and (0, 1) turned 45° land at x = ±√2/2; the turned box reaches ±√2.
    assert!(
        width.is_some_and(|width| (width - std::f32::consts::SQRT_2).abs() < 1e-5),
        "expected √2 across, got {width:?}"
    );
}

#[test]
fn a_face_laid_down_looks_at_the_plate_and_rests_on_it() {
    let (mut scene, id) = scene_with_cube();
    let object = scene.get_mut(id).expect("just added");
    object.transform.translation = Vec3::new(10.0, 20.0, 7.0);

    assert!(object.lay_face_down(Vec3::X));

    let facing = object.transform.rotation * Vec3::X;
    assert!(
        facing.abs_diff_eq(Vec3::NEG_Z, 1e-5),
        "the +X face of the cube now points down, got {facing:?}"
    );
    let bounds = object.world_bounds().expect("the cube has vertices");
    assert!(
        bounds.mins.z.abs() < 1e-5,
        "resting on z = 0, got {bounds:?}"
    );
}

#[test]
fn a_degenerate_face_turns_nothing() {
    let (mut scene, id) = scene_with_cube();
    let object = scene.get_mut(id).expect("just added");
    let before = object.transform;
    assert!(!object.lay_face_down(Vec3::ZERO));
    assert_eq!(object.transform, before);
}

#[test]
fn dragging_the_handles_moves_the_model_by_the_same_amount() {
    let (mut scene, id) = scene_with_cube();
    let object = scene.get_mut(id).expect("just added");
    let mut pivot = object.pivot();
    pivot.translation += Vec3::new(3.0, -4.0, 5.0);
    object.settle(pivot);

    assert!(
        object
            .transform
            .translation
            .abs_diff_eq(Vec3::new(3.0, -4.0, 5.0), 1e-5)
    );
}

#[test]
fn turning_the_handles_turns_the_model_about_its_centre_of_mass() {
    let (mut scene, id) = scene_with_cube();
    let object = scene.get_mut(id).expect("just added");
    object.transform.translation = Vec3::new(10.0, 20.0, 0.0);
    let before = object.pivot().translation;

    let mut pivot = object.pivot();
    pivot.rotation = Quat::from_rotation_z(FRAC_PI_2);
    object.settle(pivot);

    assert!(
        object.pivot().translation.abs_diff_eq(before, 1e-5),
        "a turn about the centre of mass leaves it where it was, got {:?}",
        object.pivot().translation
    );
    let bounds = object.world_bounds().expect("the cube has vertices");
    assert!(bounds.mins.abs_diff_eq(Vec3::new(10.0, 20.0, 0.0), 1e-5));
}

#[test]
fn stretching_the_handles_grows_the_model_about_its_centre_of_mass() {
    let (mut scene, id) = scene_with_cube();
    let object = scene.get_mut(id).expect("just added");
    let before = object.pivot().translation;

    let mut pivot = object.pivot();
    pivot.scale = Vec3::splat(4.0);
    object.settle(pivot);

    assert!(object.pivot().translation.abs_diff_eq(before, 1e-5));
    let bounds = object.world_bounds().expect("the cube has vertices");
    // A unit cube centred on (0.5, 0.5, 0.5) and grown four times spans -1.5..2.5.
    assert!(bounds.mins.abs_diff_eq(Vec3::splat(-1.5), 1e-5));
    assert!(bounds.maxs.abs_diff_eq(Vec3::splat(2.5), 1e-5));
}

#[test]
fn a_model_lands_on_the_plate_being_edited() {
    let (mut scene, first) = scene_with_cube();
    scene.add_plate();
    let second = scene.insert(crate::scene::Imported::new(
        "cube".to_owned(),
        Arc::new(unit_cube()),
        Transform::default(),
        summary(),
    ));

    assert_eq!(scene.get(first).expect("still there").plate, 0);
    assert_eq!(scene.get(second).expect("just added").plate, 1);
    assert_eq!(scene.here().count(), 1, "a plate shows only its own models");
    assert_eq!(scene.objects().len(), 2, "both are still in the project");
}

#[test]
fn a_new_plate_takes_a_name_nobody_is_using() {
    let (mut scene, _) = scene_with_cube();
    scene.add_plate();
    scene.add_plate();
    assert_eq!(scene.plates(), ["Plate 1", "Plate 2", "Plate 3"]);

    // Removing the middle one and adding another must not hand out "Plate 3" twice:
    // two tabs reading the same is what a duplicate name looks like.
    scene.remove_plate(1);
    scene.add_plate();
    assert_eq!(scene.plates(), ["Plate 1", "Plate 3", "Plate 2"]);
}

#[test]
fn switching_plates_drops_a_selection_left_behind() {
    let (mut scene, first) = scene_with_cube();
    scene.add_plate();
    assert_eq!(
        scene.selected(),
        None,
        "a new plate starts with nothing on it"
    );

    scene.show_plate(0);
    scene.select(Some(first));
    scene.show_plate(1);
    assert_eq!(scene.selected(), None);
}

#[test]
fn a_removed_plate_takes_its_models_and_shifts_the_ones_above_it() {
    let (mut scene, ground) = scene_with_cube();
    scene.add_plate();
    let middle = scene.insert(crate::scene::Imported::new(
        "middle".to_owned(),
        Arc::new(unit_cube()),
        Transform::default(),
        summary(),
    ));
    scene.add_plate();
    let top = scene.insert(crate::scene::Imported::new(
        "top".to_owned(),
        Arc::new(unit_cube()),
        Transform::default(),
        summary(),
    ));

    scene.remove_plate(1);
    assert!(scene.get(middle).is_none(), "what stood on it went with it");
    assert_eq!(scene.get(ground).expect("plate 0 is untouched").plate, 0);
    assert_eq!(
        scene.get(top).expect("still there").plate,
        1,
        "shifted down"
    );
    assert_eq!(scene.plates().len(), 2);
}

#[test]
fn a_duplicated_plate_carries_copies_of_its_models_under_a_name_of_its_own() {
    let (mut scene, original) = scene_with_cube();
    let copy = scene.duplicate_plate(0).expect("plate 0 exists");
    assert_eq!(scene.plates(), ["Plate 1", "Plate 1 copy"]);
    assert_eq!(scene.active_plate(), copy, "the copy is the plate in front");
    let copied: Vec<ObjectId> = scene.here().map(|object| object.id).collect();
    assert_eq!(copied.len(), 1);
    assert_ne!(copied[0], original, "a copy is a model of its own");
    assert_eq!(
        scene.on_plate(0).count(),
        1,
        "the original stays where it was"
    );

    scene.duplicate_plate(0);
    assert_eq!(scene.plates()[2], "Plate 1 copy 2");
    assert_eq!(scene.duplicate_plate(7), None);
}

#[test]
fn the_last_plate_cannot_be_removed() {
    let (mut scene, id) = scene_with_cube();
    scene.remove_plate(0);
    assert_eq!(
        scene.plates().len(),
        1,
        "a project always has somewhere to put a model"
    );
    assert!(scene.get(id).is_some());
}

#[test]
fn a_model_moved_to_another_plate_leaves_this_one() {
    let (mut scene, id) = scene_with_cube();
    scene.add_plate();
    scene.show_plate(0);
    scene.select(Some(id));

    scene.move_to_plate(id, 1);
    assert_eq!(scene.get(id).expect("still there").plate, 1);
    assert_eq!(scene.here().count(), 0);
    assert_eq!(
        scene.selected(),
        None,
        "it is no longer in front of the user"
    );
}

#[test]
fn only_what_stands_on_a_plate_counts_as_printable() {
    let (mut scene, id) = scene_with_cube();
    assert!(scene.has_printable(0));
    scene.get_mut(id).expect("still there").visible = false;
    assert!(!scene.has_printable(0), "a hidden model is not printed");
    assert_eq!(scene.here().count(), 1, "but it is still listed");
}

/// A plate with `count` cubes on it, and the identifier of each.
fn plate_of(count: usize) -> (Scene, Vec<ObjectId>) {
    let mut scene = Scene::default();
    let ids = (0..count)
        .map(|index| {
            scene.insert(crate::scene::Imported::new(
                format!("cube {index}"),
                Arc::new(unit_cube()),
                Transform::default(),
                summary(),
            ))
        })
        .collect();
    scene.clear_selection();
    (scene, ids)
}

#[test]
fn with_nothing_picked_a_tool_takes_the_whole_plate() {
    let (scene, _) = plate_of(3);
    assert!(!scene.has_selection());
    assert_eq!(scene.target_count(), 3);
}

#[test]
fn with_something_picked_a_tool_takes_only_that() {
    let (mut scene, ids) = plate_of(3);
    scene.select(Some(ids[1]));
    assert_eq!(scene.target_count(), 1);

    scene.toggle_selected(ids[2]);
    let targets: Vec<ObjectId> = scene.targets().map(|object| object.id).collect();
    assert_eq!(
        targets,
        [ids[1], ids[2]],
        "in the order they stand on the plate"
    );
}

/// Shift-click in a list takes everything between what was last picked and this,
/// the way every list on the desktop reads it.
#[test]
fn a_shift_click_takes_the_span_between_two_rows() {
    let (mut scene, ids) = plate_of(5);
    scene.select(Some(ids[1]));
    scene.select_span_to(ids[3]);
    assert_eq!(scene.selection(), [ids[1], ids[2], ids[3]]);

    scene.select_span_to(ids[0]);
    assert_eq!(
        scene.selection(),
        [ids[0], ids[1]],
        "a span is measured from the same row, up the list as well as down"
    );
}

#[test]
fn a_span_from_nothing_picks_the_row_it_landed_on() {
    let (mut scene, ids) = plate_of(3);
    scene.select_span_to(ids[2]);
    assert_eq!(scene.selection(), [ids[2]]);
}

/// A span starts at the row a modifier-click landed on, so holding cmd and then
/// shift reaches from the one just added.
#[test]
fn a_modifier_click_moves_where_the_next_span_starts() {
    let (mut scene, ids) = plate_of(4);
    scene.select(Some(ids[0]));
    scene.toggle_selected(ids[3]);
    scene.select_span_to(ids[2]);
    assert_eq!(scene.selection(), [ids[2], ids[3]]);
}

#[test]
fn a_modifier_click_on_something_already_picked_takes_it_back_out() {
    let (mut scene, ids) = plate_of(3);
    scene.select(Some(ids[0]));
    scene.toggle_selected(ids[1]);
    scene.toggle_selected(ids[0]);

    assert_eq!(scene.selection(), [ids[1]]);
    assert_eq!(
        scene.selected(),
        Some(ids[1]),
        "the last one left is the primary"
    );
}

#[test]
fn a_hidden_model_is_no_target_even_when_it_is_picked() {
    let (mut scene, ids) = plate_of(2);
    scene.select_here();
    scene.get_mut(ids[0]).expect("still there").visible = false;

    assert_eq!(
        scene.target_count(),
        1,
        "a tool does not touch what is not drawn"
    );
    assert_eq!(
        scene.selection().len(),
        2,
        "but the pick is still the user's"
    );
}

#[test]
fn a_pick_does_not_follow_the_user_to_another_plate() {
    let (mut scene, _) = plate_of(2);
    scene.select_here();
    scene.add_plate();

    assert!(scene.selection().is_empty());
    assert_eq!(scene.target_count(), 0, "and the new plate is empty");
}

#[test]
fn deleting_takes_every_picked_model_at_once() {
    let (mut scene, ids) = plate_of(3);
    scene.select(Some(ids[0]));
    scene.toggle_selected(ids[2]);

    assert_eq!(scene.remove_selected(), 2);
    assert_eq!(scene.objects().len(), 1);
    assert_eq!(scene.objects()[0].id, ids[1]);
    assert!(scene.selection().is_empty());
}

#[test]
fn what_a_tool_says_it_will_do_follows_what_is_picked() {
    let (mut scene, ids) = plate_of(3);
    assert_eq!(scene.scope(), "all 3 models on the plate");

    scene.select(Some(ids[0]));
    assert_eq!(scene.scope(), "the selected model");

    scene.toggle_selected(ids[1]);
    assert_eq!(scene.scope(), "the 2 selected models");
}

#[test]
fn inserting_selects_the_new_object() {
    let (scene, id) = scene_with_cube();
    assert_eq!(scene.selected(), Some(id));
    assert_eq!(scene.objects().len(), 1);
}

#[test]
fn removing_the_selected_object_clears_the_selection() {
    let (mut scene, id) = scene_with_cube();
    scene.remove(id);
    assert!(scene.is_empty());
    assert_eq!(scene.selected(), None);
}

#[test]
fn identifiers_are_never_reused() {
    let (mut scene, first) = scene_with_cube();
    scene.remove(first);
    let second = scene.insert(crate::scene::Imported::new(
        "cube".to_owned(),
        Arc::new(unit_cube()),
        Transform::default(),
        summary(),
    ));
    assert_ne!(first, second);
    assert!(scene.get(first).is_none());
}

#[test]
fn selecting_a_missing_object_clears_the_selection() {
    let (mut scene, id) = scene_with_cube();
    scene.remove(id);
    scene.select(Some(id));
    assert_eq!(scene.selected(), None);
}

#[test]
fn world_bounds_follow_the_transform() {
    let (mut scene, id) = scene_with_cube();
    scene.objects_mut()[0].transform = Transform {
        translation: Vec3::new(10.0, 20.0, 0.0),
        rotation: Quat::IDENTITY,
        scale: Vec3::splat(2.0),
        ..Transform::default()
    };

    let bounds = scene.get(id).and_then(SceneObject::world_bounds);
    let bounds = bounds.expect("the cube has vertices");
    assert!(bounds.mins.abs_diff_eq(Vec3::new(10.0, 20.0, 0.0), 1e-5));
    assert!(bounds.maxs.abs_diff_eq(Vec3::new(12.0, 22.0, 2.0), 1e-5));
}

#[test]
fn rotating_the_cube_keeps_its_footprint_square() {
    let (mut scene, _) = scene_with_cube();
    scene.objects_mut()[0].transform = Transform {
        rotation: Quat::from_rotation_z(FRAC_PI_2),
        ..Transform::default()
    };

    // A quarter turn about Z maps the unit cube onto x in -1..0, y in 0..1.
    let bounds = scene.world_bounds().expect("one object is visible");
    let size = bounds.maxs - bounds.mins;
    assert!((size.x - 1.0).abs() < 1e-5, "a quarter turn keeps width 1");
    assert!((size.y - 1.0).abs() < 1e-5, "a quarter turn keeps depth 1");
}

#[test]
fn hidden_objects_are_outside_the_world_bounds() {
    let (mut scene, _) = scene_with_cube();
    scene.objects_mut()[0].visible = false;
    assert!(scene.world_bounds().is_none());
}

#[test]
fn empty_scene_has_no_bounds() {
    let scene = Scene::default();
    assert!(scene.world_bounds().is_none());
}

#[test]
fn a_copy_carries_the_supports_of_what_it_was_copied_from() {
    let (mut scene, id) = scene_with_cube();
    if let Some(object) = scene.get_mut(id) {
        object
            .supports
            .add(Vec3::new(0.5, 0.5, 1.0), Transform::default(), 0);
    }

    let copy = scene
        .duplicate(id, Vec3::new(10.0, 0.0, 0.0))
        .expect("the cube is on the plate");
    assert_eq!(
        scene.selected(),
        Some(copy),
        "a copy is what you go on to move"
    );
    assert_ne!(copy, id);

    let copied = scene.get(copy).expect("the copy is on the plate");
    assert_eq!(copied.supports.point_count(), 1);
    assert_eq!(copied.transform.translation, Vec3::new(10.0, 0.0, 0.0));
}

#[test]
fn an_array_lays_out_every_cell_but_the_one_already_filled() {
    let (mut scene, id) = scene_with_cube();
    let copies = scene.array(id, 3, 2, 1.0, Vec2::new(100.0, 100.0));

    assert_eq!(copies.len(), 5, "six cells, one of them the original");
    assert_eq!(scene.objects().len(), 6);

    // The unit cube with a 1 mm gap steps 2 mm, so the far corner stands at (4, 2).
    let last = scene.get(copies[4]).expect("the last copy is on the plate");
    assert_eq!(last.transform.translation, Vec3::new(4.0, 2.0, 0.0));
}

#[test]
fn an_array_that_would_run_off_the_plate_is_slid_back_onto_it() {
    let (mut scene, id) = scene_with_cube();
    scene.objects_mut()[0].transform.translation = Vec3::new(8.0, 0.0, 0.0);

    let copies = scene.array(id, 3, 1, 1.0, Vec2::new(10.0, 10.0));

    // Three unit cubes two millimetres apart span 5 mm, so the grid starts at 5 mm.
    let left = scene.get(id).expect("the original is on the plate");
    assert_eq!(left.transform.translation, Vec3::new(5.0, 0.0, 0.0));
    let right = scene.get(copies[1]).expect("the far copy is on the plate");
    assert_eq!(right.transform.translation, Vec3::new(9.0, 0.0, 0.0));
    for object in scene.objects() {
        let bounds = object.world_bounds().expect("a cube has vertices");
        assert!(bounds.maxs.x <= 10.0, "{bounds:?} hangs off the plate");
    }
}

#[test]
fn an_array_with_no_room_on_the_plate_starts_at_its_near_corner() {
    let (mut scene, id) = scene_with_cube();
    scene.objects_mut()[0].transform.translation = Vec3::new(3.0, 3.0, 0.0);

    scene.array(id, 5, 1, 1.0, Vec2::new(4.0, 4.0));

    let left = scene.get(id).expect("the original is on the plate");
    assert_eq!(
        left.transform.translation,
        Vec3::new(0.0, 3.0, 0.0),
        "a grid that cannot fit still starts on the plate"
    );
}

#[test]
fn an_array_of_something_that_is_not_there_lays_out_nothing() {
    let (mut scene, id) = scene_with_cube();
    scene.remove(id);
    assert!(
        scene
            .array(id, 2, 2, 1.0, Vec2::new(100.0, 100.0))
            .is_empty()
    );
}

#[test]
fn mirroring_leaves_the_model_where_it_stood() {
    let (mut scene, id) = scene_with_cube();
    scene.objects_mut()[0].transform.translation = Vec3::new(10.0, 20.0, 0.0);
    let before = scene.world_bounds().expect("the cube has vertices");

    assert!(scene.mirror(id, Axis::X));
    let after = scene
        .world_bounds()
        .expect("a mirrored cube still has vertices");
    assert!(after.mins.abs_diff_eq(before.mins, 1e-5));
    assert!(after.maxs.abs_diff_eq(before.maxs, 1e-5));
}

#[test]
fn a_mirrored_model_is_still_solid() {
    let (mut scene, id) = scene_with_cube();
    assert!(scene.mirror(id, Axis::Y));

    let object = scene.get(id).expect("the cube is on the plate");
    let placed = core_geometry::transform_mesh(&object.mesh, object.transform);
    assert!(
        core_geometry::signed_volume(&placed) > 0.0,
        "the flip reverses winding, and the faces are put back the right way round"
    );
}
