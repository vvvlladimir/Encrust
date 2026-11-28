use core_geometry::Vec3;
use core_supports::{ModelSupports, SupportPoint};

use super::*;
use crate::project::state::{Captured, capture};
use crate::scene::fixtures::scene_of_boxes;
use crate::slicing::Slicing;
use crate::state::Tools;

/// A plate of one 10 mm box whose one support stands in group `group` of `tools`.
fn shown_plate(tools: &Tools, group: u16) -> Project {
    let mut scene = scene_of_boxes(&[(Vec3::ZERO, Vec3::splat(10.0))]);
    let object = &mut scene.objects_mut()[0];
    object.supports = ModelSupports::restore(
        vec![SupportPoint::new(Vec3::new(5.0, 5.0, 0.0)).in_group(group)],
        Default::default(),
        Default::default(),
        Vec::new(),
    );
    let slicing = Slicing::default();
    capture(Captured {
        scene: &scene,
        slicing: &slicing,
        tools,
    })
}

/// The user's plate: one box of their own, the window's one support group.
fn users_plate() -> (Scene, Tools) {
    let scene = scene_of_boxes(&[(Vec3::new(30.0, 30.0, 0.0), Vec3::splat(40.0))]);
    (scene, Tools::default())
}

#[test]
fn a_plate_shown_stands_beside_the_users_model_under_a_fresh_id() {
    let plate = shown_plate(&Tools::default(), 0);
    let (mut scene, mut tools) = users_plate();
    let theirs = scene.objects()[0].id;

    let ids = insert(plate, &mut scene, &mut tools.supports);

    assert_eq!(ids.len(), 1, "one object on the plate shown");
    assert_eq!(scene.objects().len(), 2, "the user's box stays");
    assert_ne!(ids[0], theirs, "the shown box takes an id of its own");
}

#[test]
fn a_group_the_window_lacks_is_added_and_the_support_moves_into_it() {
    let mut saved = Tools::default();
    saved.supports.add_group();
    let name = saved.supports.groups[1].name.clone();
    let plate = shown_plate(&saved, 1);
    let (mut scene, mut tools) = users_plate();
    tools.supports.groups[0].name = "the user's own".into();

    let ids = insert(plate, &mut scene, &mut tools.supports);

    let shown = scene.get(ids[0]).expect("the shown box stands");
    let group = shown.supports.points()[0].group;
    assert_eq!(
        tools.supports.groups[group as usize].name, name,
        "the support is in the window's group of its own group's name"
    );
    assert_eq!(
        tools.supports.groups.len(),
        3,
        "both of the plate's groups were new"
    );
}

#[test]
fn a_group_of_a_name_the_window_has_builds_to_the_plates_profile() {
    let mut saved = Tools::default();
    saved.supports.profile.density = 2.0;
    let plate = shown_plate(&saved, 0);
    let (mut scene, mut tools) = users_plate();

    let ids = insert(plate, &mut scene, &mut tools.supports);

    assert_eq!(
        tools.supports.groups.len(),
        1,
        "the group of that name is shared"
    );
    let shown = scene.get(ids[0]).expect("the shown box stands");
    let own = shown
        .own
        .supports
        .get(&0)
        .expect("the plate's numbers stay the box's own");
    assert!(
        (own.density - 2.0).abs() < 1e-6,
        "density 2 as checked, got {}",
        own.density
    );
}
