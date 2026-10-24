use super::*;
use crate::SupportPoint;
use core_geometry::Quat;
use core_geometry::{Bvh, Mesh, Region, Scalar, Transform, Vec3};
use printer_profiles::SupportProfile;
use std::f32::consts::FRAC_PI_2;
use std::sync::Arc;

fn profile() -> SupportProfile {
    crate::fixtures::profile()
}

/// An empty model, so every column runs all the way to the plate.
fn nothing() -> Mesh {
    Mesh::default()
}

/// Rebuilds against a model, building its hierarchy the way the scene does.
fn refresh(supports: &mut ModelSupports, model: &Mesh, transform: Transform) {
    supports.refresh(
        model,
        &Bvh::build(model),
        transform,
        std::slice::from_ref(&profile()),
    );
}

/// Where the one support of `supports` meets the model, in plate coordinates.
fn contact_of(supports: &ModelSupports) -> Vec3 {
    supports.trees()[0]
        .nodes()
        .iter()
        .find(|node| node.is_leaf())
        .expect("a support carries a tip")
        .position
}

fn with_one_support(transform: Transform) -> ModelSupports {
    let mut supports = ModelSupports::default();
    supports.add(Vec3::new(10.0, 10.0, 20.0), transform, 0);
    refresh(&mut supports, &nothing(), transform);
    supports
}

#[test]
fn a_new_object_has_no_supports_and_no_mesh() {
    let supports = ModelSupports::default();
    assert!(supports.is_empty());
    assert!(supports.meshes().is_none());
    assert!(supports.trees().is_empty());
}

#[test]
fn clearing_one_group_leaves_the_others_and_their_numbers() {
    let mut supports = ModelSupports::default();
    for (x, group) in [(1.0, 0), (2.0, 1), (3.0, 2), (4.0, 1)] {
        supports.add(Vec3::new(x, 0.0, 10.0), Transform::default(), group);
    }

    assert_eq!(supports.clear_group(1), 2);

    let left: Vec<u16> = supports.points().iter().map(|point| point.group).collect();
    assert_eq!(left, vec![0, 2], "group 2 is still group 2");
    assert_eq!(supports.clear_group(1), 0, "nothing is left in it to take");
}

#[test]
fn clearing_takes_the_supports_a_hand_froze_as_well() {
    let mut supports = with_one_support(Transform::default());
    supports
        .freeze(0, Transform::default())
        .expect("the one tree is there to freeze");
    assert_eq!(supports.frozen().len(), 1);

    supports.clear();
    refresh(&mut supports, &nothing(), Transform::default());

    assert!(supports.frozen().is_empty());
    assert_eq!(supports.standing(), 0, "nothing is left standing");
}

#[test]
fn trees_grown_elsewhere_are_built_without_growing_again() {
    let mut supports = ModelSupports::default();
    supports.add(Vec3::new(10.0, 10.0, 20.0), Transform::default(), 0);
    let table = [profile()];
    let growth = supports
        .growth(Transform::default(), &table)
        .expect("a new point has nothing grown for it");
    let model = nothing();
    let bvh = Bvh::build(&model);

    assert!(supports.take_grown(growth.grow(&model, &model, &bvh)));
    assert!(!supports.needs_growing(Transform::default(), &table));
    supports.refresh(&model, &bvh, Transform::default(), &table);
    assert_eq!(supports.standing(), 1);
}

#[test]
fn trees_grown_elsewhere_come_back_meshed() {
    let mut supports = ModelSupports::default();
    supports.add(Vec3::new(10.0, 10.0, 20.0), Transform::default(), 0);
    let table = [profile()];
    let growth = supports
        .growth(Transform::default(), &table)
        .expect("a new point has nothing grown for it");
    let model = nothing();
    let bvh = Bvh::build(&model);

    assert!(supports.take_grown(growth.grow(&model, &model, &bvh)));
    assert!(
        supports.meshes().is_some(),
        "the worker meshed them, so the drawing thread has nothing to build"
    );
}

#[test]
fn a_frozen_support_edited_while_the_others_grow_is_meshed_again() {
    let mut supports = three_apart();
    supports.freeze(tree_at(&supports, 0.0), Transform::default());
    supports.add(Vec3::new(90.0, 0.0, 20.0), Transform::default(), 0);
    let table = [profile()];
    let growth = supports
        .growth(Transform::default(), &table)
        .expect("a new point has nothing grown for it");
    supports
        .frozen_mut(0)
        .expect("the support frozen above")
        .set_radius_mm(1.0);
    let model = nothing();
    let bvh = Bvh::build(&model);

    assert!(supports.take_grown(growth.grow(&model, &model, &bvh)));
    assert!(
        supports.meshes().is_none(),
        "the meshes show the frozen support as it was, so they are built again"
    );
}

#[test]
fn points_saved_before_their_trees_grew_are_grown_when_stood() {
    let mut supports = ModelSupports::restore(
        vec![SupportPoint::new(Vec3::new(10.0, 10.0, 20.0))],
        Region::default(),
        Region::default(),
        Vec::new(),
    );
    let model = nothing();
    supports.stand(
        &[],
        &model,
        &model,
        &Bvh::build(&model),
        Transform::default(),
        &[profile()],
    );
    assert_eq!(supports.standing(), 1);
}

#[test]
fn trees_grown_for_points_since_changed_are_turned_away() {
    let mut supports = ModelSupports::default();
    supports.add(Vec3::new(10.0, 10.0, 20.0), Transform::default(), 0);
    let table = [profile()];
    let growth = supports
        .growth(Transform::default(), &table)
        .expect("a new point has nothing grown for it");
    supports.add(Vec3::new(20.0, 10.0, 20.0), Transform::default(), 0);
    let model = nothing();
    let bvh = Bvh::build(&model);

    assert!(!supports.take_grown(growth.grow(&model, &model, &bvh)));
    assert!(supports.needs_growing(Transform::default(), &table));
}

#[test]
fn a_model_moved_off_its_trees_has_nothing_built_until_they_grow_again() {
    let mut supports = with_one_support(Transform::default());
    let moved = Transform::from_translation(Vec3::new(5.0, 0.0, 0.0));
    let table = [profile()];

    assert!(!supports.outgrown(Transform::default(), &table));
    assert!(
        supports.meshes().is_some(),
        "where it stood, the build holds"
    );
    assert!(supports.outgrown(moved, &table));
    assert!(
        supports.meshes().is_none(),
        "moved, nothing built is current"
    );
    assert!(
        !supports.drawn().is_empty(),
        "the last columns are still drawn"
    );
}

#[test]
fn a_placed_support_becomes_a_column_with_geometry() {
    let supports = with_one_support(Transform::default());
    assert_eq!(supports.point_count(), 1);
    assert_eq!(supports.standing(), 1);
    assert!(!supports.meshes().expect("a column was built")[0].is_empty());
}

#[test]
fn a_contact_is_stored_in_the_models_own_space() {
    let transform = Transform::from_translation(Vec3::new(50.0, 0.0, 0.0));
    let supports = with_one_support(transform);

    // Clicked at x = 10 on a model standing at x = 50, so the anchor is 40 mm back
    // along the model's own x, and comes out at x = 10 again when it is rebuilt.
    assert!(contact_of(&supports).abs_diff_eq(Vec3::new(10.0, 10.0, 20.0), 1e-4));
}

#[test]
fn a_support_follows_the_model_that_is_moved_under_it() {
    let mut supports = with_one_support(Transform::default());
    let moved = Transform::from_translation(Vec3::new(5.0, 0.0, 0.0));
    refresh(&mut supports, &nothing(), moved);

    assert!(
        (contact_of(&supports).x - 15.0).abs() < 1e-4,
        "the anchor travelled with the model"
    );
}

#[test]
fn a_support_follows_the_model_that_is_turned_under_it() {
    let mut supports = with_one_support(Transform::default());
    let turned = Transform {
        rotation: Quat::from_rotation_z(FRAC_PI_2),
        ..Transform::default()
    };
    refresh(&mut supports, &nothing(), turned);

    // A quarter turn about z maps (10, 10) onto (-10, 10).
    let contact = contact_of(&supports);
    assert!(
        contact.abs_diff_eq(Vec3::new(-10.0, 10.0, 20.0), 1e-4),
        "expected the anchor to turn with the model, got {contact}"
    );
}

#[test]
fn a_thicker_profile_rebuilds_the_mesh() {
    let mut supports = with_one_support(Transform::default());
    let thin = supports.meshes().expect("built")[0].clone();

    supports.refresh(
        &nothing(),
        &Bvh::default(),
        Transform::default(),
        std::slice::from_ref(&SupportProfile::shipped("heavy").expect("the heavy preset ships")),
    );
    let thick = &supports.meshes().expect("rebuilt")[0];
    assert!(
        !Arc::ptr_eq(&thin, thick),
        "a new profile must produce a new mesh"
    );
}

#[test]
fn an_edit_leaves_the_last_columns_to_draw_until_they_are_rebuilt() {
    let transform = Transform::default();
    let mut supports = with_one_support(transform);
    let before = supports.drawn().to_vec();
    assert!(before.iter().any(|mesh| !mesh.is_empty()), "one column");

    supports.add(Vec3::new(3.0, 3.0, 10.0), transform, 0);
    assert!(supports.meshes().is_none(), "what is built is out of date");
    assert_eq!(
        supports.drawn().len(),
        before.len(),
        "and the viewport still has the last columns to draw"
    );
    assert!(
        supports
            .drawn()
            .iter()
            .zip(&before)
            .all(|(now, then)| Arc::ptr_eq(now, then))
    );

    refresh(&mut supports, &nothing(), transform);
    let faces = |meshes: &[Arc<Mesh>]| meshes.iter().map(|mesh| mesh.faces.len()).sum::<usize>();
    assert!(
        faces(supports.drawn()) > faces(&before),
        "the rebuild draws the second column as well"
    );
}

#[test]
fn nothing_changing_keeps_the_mesh_the_viewport_has_cached() {
    let mut supports = with_one_support(Transform::default());
    let first = supports.meshes().expect("built")[0].clone();

    refresh(&mut supports, &nothing(), Transform::default());
    assert!(
        Arc::ptr_eq(&first, &supports.meshes().expect("still built")[0]),
        "an unchanged object must not churn the GPU cache every frame"
    );
}

#[test]
fn dropping_a_group_takes_its_supports_with_it() {
    let mut supports = ModelSupports::default();
    supports.add(Vec3::new(10.0, 10.0, 20.0), Transform::default(), 0);
    supports.add(Vec3::new(20.0, 10.0, 20.0), Transform::default(), 1);

    assert_eq!(supports.drop_group(1), 1, "the group held one support");
    assert_eq!(supports.point_count(), 1);
    assert!(
        supports.points().iter().all(|point| point.group == 0),
        "the support of the group that stayed is untouched"
    );
}

#[test]
fn dropping_a_group_shifts_the_groups_above_it_down() {
    let mut supports = ModelSupports::default();
    supports.add(Vec3::new(10.0, 10.0, 20.0), Transform::default(), 1);
    supports.add(Vec3::new(20.0, 10.0, 20.0), Transform::default(), 2);

    assert_eq!(supports.drop_group(1), 1);
    assert_eq!(
        supports.points()[0].group,
        1,
        "the group above the one taken away moved down into its number"
    );
}

#[test]
fn removing_a_point_drops_its_support() {
    let mut supports = with_one_support(Transform::default());
    supports.remove(0);
    refresh(&mut supports, &nothing(), Transform::default());

    assert!(supports.is_empty());
    assert!(supports.trees().is_empty());
    assert!(
        supports
            .meshes()
            .expect("an empty mesh is still built")
            .iter()
            .all(|mesh| mesh.is_empty())
    );
}

#[test]
fn removing_a_point_that_is_not_there_changes_nothing() {
    let mut supports = with_one_support(Transform::default());
    supports.remove(7);
    assert_eq!(supports.point_count(), 1);
}

#[test]
fn clearing_takes_every_support_away() {
    let mut supports = with_one_support(Transform::default());
    supports.add(Vec3::new(20.0, 20.0, 20.0), Transform::default(), 0);
    supports.clear();
    assert!(supports.is_empty());
}

#[test]
fn a_click_on_a_flattened_model_places_nothing() {
    let flat = Transform {
        scale: Vec3::new(1.0, 1.0, 0.0),
        ..Transform::default()
    };
    let mut supports = ModelSupports::default();
    supports.add(Vec3::new(10.0, 10.0, 20.0), flat, 0);
    assert!(supports.is_empty());
}

/// Three supports far enough apart never to share a trunk, built on an empty model.
fn three_apart() -> ModelSupports {
    let mut supports = ModelSupports::default();
    for x in [0.0, 30.0, 60.0] {
        supports.add(Vec3::new(x, 0.0, 20.0), Transform::default(), 0);
    }
    refresh(&mut supports, &nothing(), Transform::default());
    supports
}

/// Which grown tree holds the tip at `x`.
fn tree_at(supports: &ModelSupports, x: Scalar) -> usize {
    supports.trees()[..supports.trees().len() - supports.frozen().len()]
        .iter()
        .position(|tree| tree.contacts().any(|tip| (tip.x - x).abs() < 1e-3))
        .expect("a support stands at that x")
}

#[test]
fn freezing_one_support_leaves_the_others_holding_their_own_points() {
    let mut supports = three_apart();
    supports.freeze(tree_at(&supports, 0.0), Transform::default());
    refresh(&mut supports, &nothing(), Transform::default());

    supports.freeze(tree_at(&supports, 60.0), Transform::default());
    let left: Vec<Scalar> = supports
        .points()
        .iter()
        .map(|point| point.contact.x)
        .collect();
    assert_eq!(left, vec![30.0], "the second freeze took the point it held");
}

#[test]
fn editing_a_frozen_support_does_not_grow_the_others_again() {
    let mut supports = three_apart();
    supports.freeze(tree_at(&supports, 0.0), Transform::default());
    // A slab over the plate the grown supports would now have to stand on: growing them
    // again would land them on it, reusing them leaves them on the plate.
    let slab = crate::fixtures::box_mesh(Vec3::new(-50.0, -50.0, 5.0), Vec3::new(100.0, 50.0, 6.0));
    let bvh = Bvh::build(&slab);
    let profile = profile();
    let table = std::slice::from_ref(&profile);
    supports.refresh(&slab, &bvh, Transform::default(), table);
    supports.frozen_mut(0);
    supports.refresh(&slab, &bvh, Transform::default(), table);

    let on_plate = supports.trees()[..2]
        .iter()
        .all(|tree| !tree.landing().on_model);
    assert!(on_plate, "only the frozen support was built again");
}

#[test]
fn a_new_table_alone_is_told_apart_from_other_edits() {
    let mut supports = three_apart();
    let mut thicker = profile();
    thicker.middle.diameter_mm += 0.5;
    let table = std::slice::from_ref(&thicker);
    assert!(supports.retuned(Transform::default(), table));

    let moved = Transform::from_translation(Vec3::X);
    assert!(!supports.retuned(moved, table), "a move is no retune");
    supports.add(Vec3::new(90.0, 0.0, 20.0), Transform::default(), 0);
    assert!(
        !supports.retuned(Transform::default(), table),
        "nor is a new point"
    );
}
