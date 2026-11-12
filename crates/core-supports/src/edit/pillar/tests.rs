use super::*;
use crate::edit::{carried_held, tip_carried};
use crate::fixtures::{box_mesh, profile, trees_on};
use crate::{Profiles, SupportPoint};
use core_geometry::{Bvh, Mesh, Transform};

/// A slab whose underside at z = 10 runs over x 0..20, and a block on the plate under its first
/// 8 mm whose top is at z = 6.5: a contact over the block is held by a small pillar.
fn slab_over_a_block() -> Mesh {
    let mut model = box_mesh(Vec3::new(0.0, -5.0, 10.0), Vec3::new(20.0, 5.0, 12.0));
    model.append(&box_mesh(
        Vec3::new(0.0, -5.0, 0.0),
        Vec3::new(8.0, 5.0, 6.5),
    ));
    model
}

/// [`slab_over_a_block`] with fins 0.7 mm apart hanging 1.5 mm round x = 4, so a contact there
/// has no room for a head and is held by a small pillar of its own search.
fn slot_over_a_block() -> Mesh {
    let mut model = slab_over_a_block();
    for x in [3.55, 4.35] {
        model.append(&box_mesh(
            Vec3::new(x, -5.0, 8.5),
            Vec3::new(x + 0.1, 5.0, 10.0),
        ));
    }
    model
}

fn grown_at(model: &Mesh, contact: Vec3) -> SupportTree {
    trees_on(
        &[SupportPoint::new(contact)],
        model,
        Transform::default(),
        Profiles::single(&profile()),
    )
    .pop()
    .expect("a contact under the slab is held")
}

fn tip_of(tree: &SupportTree) -> usize {
    tree.nodes()
        .iter()
        .position(crate::TreeNode::is_leaf)
        .expect("a support has a tip")
}

#[test]
fn a_small_pillar_tip_carried_over_open_plate_becomes_the_support_growing_builds_there() {
    let model = slab_over_a_block();
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let fixture = profile();
    let profiles = Profiles::single(&fixture);
    let pillar = grown_at(&model, Vec3::new(4.0, 0.0, 10.0));
    assert!(is_small(&pillar, &fixture), "over the block it grows thin");

    let to = Vec3::new(14.0, 0.0, 10.0);
    let moved = tip_carried(&pillar, tip_of(&pillar), to, &placed, profiles)
        .expect("a whole support stands on the plate under x = 14");

    assert_eq!(
        moved,
        grown_at(&model, to),
        "neither a small pillar nor half of one: the support grown from that contact"
    );
    assert!(!is_small(&moved, &fixture) && !moved.landing().on_model);
}

#[test]
fn a_headless_small_pillar_tip_carried_out_of_its_slot_becomes_a_whole_support() {
    let model = slot_over_a_block();
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let fixture = profile();
    let pillar = grown_at(&model, Vec3::new(4.0, 0.0, 10.0));
    assert!(is_small(&pillar, &fixture), "in the slot it grows thin");

    let to = Vec3::new(14.0, 0.0, 10.0);
    let moved = tip_carried(
        &pillar,
        tip_of(&pillar),
        to,
        &placed,
        Profiles::single(&fixture),
    )
    .expect("a whole support stands on the plate under x = 14");

    assert_eq!(moved, grown_at(&model, to));
}

#[test]
fn a_small_pillar_tip_carried_along_the_block_stays_a_small_pillar() {
    let model = slab_over_a_block();
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let fixture = profile();
    let pillar = grown_at(&model, Vec3::new(4.0, 0.0, 10.0));

    let to = Vec3::new(2.0, 0.0, 10.0);
    let moved = tip_carried(
        &pillar,
        tip_of(&pillar),
        to,
        &placed,
        Profiles::single(&fixture),
    )
    .expect("a small pillar stands over the block at x = 2");

    assert!(is_small(&moved, &fixture), "still built thin");
    let base = moved.landing().base;
    assert!(
        (base.z - 6.5).abs() < 1e-3,
        "it stands on the block's top at z = 6.5, got {base}"
    );
}

#[test]
fn a_small_pillar_carried_by_its_trunk_keeps_holding_the_slab() {
    let model = slab_over_a_block();
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let fixture = profile();
    let profiles = Profiles::single(&fixture);
    let standing = [grown_at(&model, Vec3::new(4.0, 0.0, 10.0))];

    let held = [(0, vec![Part::Trunk])];
    let step = Vec3::new(10.0, 0.0, 0.0);
    let carried =
        carried_held(&standing, &[], &held, step, &placed, profiles).expect("the carry is built");

    assert_eq!(carried.refused, None);
    let (_, moved) = &carried.moved[0];
    let contact = moved.nodes()[tip_of(moved)].position;
    assert!(
        contact.abs_diff_eq(Vec3::new(14.0, 0.0, 10.0), 1e-3),
        "its tip still holds the slab's underside, 10 mm along, got {contact}"
    );
    assert_eq!(*moved, grown_at(&model, contact));
}

#[test]
fn a_small_pillar_carried_out_from_under_everything_is_refused_where_it_stood() {
    let model = slab_over_a_block();
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let fixture = profile();
    let standing = [grown_at(&model, Vec3::new(4.0, 0.0, 10.0))];

    let held = [(0, vec![Part::Foot])];
    let step = Vec3::new(30.0, 0.0, 0.0);
    let carried = carried_held(
        &standing,
        &[],
        &held,
        step,
        &placed,
        Profiles::single(&fixture),
    )
    .expect("the carry is built");

    assert_eq!(carried.refused, Some(Refused::NoHold));
    assert_eq!(carried.moved[0].1, standing[0], "it waits where it stood");
}
