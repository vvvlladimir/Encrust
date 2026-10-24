use super::*;
use crate::SupportPoint;
use crate::fixtures::{box_mesh, profile, ramp, trees_on};
use crate::mesher::{is_small, mesh_trees};
use core_geometry::{Bvh, Mesh, Transform};

/// A slab's underside at z = 10 with a slot 0.7 mm wide along y hanging 1.5 mm from it round a
/// contact at the origin, too narrow anywhere for a head, over a block whose top is at z = 6
/// from `block_y` along +y.
fn in_a_slot_over_a_block(block_y: Scalar) -> Mesh {
    let mut model = box_mesh(Vec3::new(-5.0, -5.0, 10.0), Vec3::new(5.0, 5.0, 12.0));
    for x in [-0.45, 0.35] {
        model.append(&box_mesh(
            Vec3::new(x, -5.0, 8.5),
            Vec3::new(x + 0.1, 5.0, 10.0),
        ));
    }
    model.append(&box_mesh(
        Vec3::new(-5.0, block_y, 0.0),
        Vec3::new(5.0, block_y + 4.0, 6.0),
    ));
    model
}

fn trees_under(model: &Mesh, fixture: &SupportProfile) -> Vec<SupportTree> {
    trees_on(
        &[SupportPoint::new(Vec3::ZERO.with_z(10.0))],
        model,
        Transform::default(),
        Profiles::single(fixture),
    )
}

#[test]
fn a_contact_with_no_room_for_a_head_is_held_by_a_small_pillar() {
    let model = in_a_slot_over_a_block(-2.0);
    let mut regular = profile();
    regular.small_pillar.enabled = false;
    assert!(
        trees_under(&model, &regular).is_empty(),
        "the slot refuses every head, and without small pillars nothing holds the contact"
    );

    let fixture = profile();
    let trees = trees_under(&model, &fixture);
    assert_eq!(trees.len(), 1, "a small pillar holds it");
    let tree = &trees[0];
    assert!(is_small(tree, &fixture), "built thin");
    assert!(
        tree.landing()
            .base
            .abs_diff_eq(Vec3::ZERO.with_z(6.0), 1e-3),
        "straight down onto the block, got {}",
        tree.landing().base
    );
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    assert!(sound(tree, Profiles::single(&fixture), &placed));
}

#[test]
fn a_small_pillar_leans_for_a_face_it_cannot_reach_straight_down() {
    // Nothing under the contact but the plate; the block starts a millimetre along the slot.
    let model = in_a_slot_over_a_block(1.0);
    let fixture = profile();
    let trees = trees_under(&model, &fixture);
    assert_eq!(trees.len(), 1, "a leaning small pillar holds it");
    let tree = &trees[0];
    assert!(is_small(tree, &fixture), "built thin");
    let base = tree.landing().base;
    assert!(
        (base.z - 6.0).abs() < 1e-3 && base.y >= 1.0,
        "it stands on the block's top, got {base}"
    );

    let nodes = tree.nodes();
    let tip = nodes.iter().find(|node| node.is_leaf()).expect("one tip");
    let under_tip = nodes[tip.parent.expect("a bend under the tip")].position;
    assert!(
        (under_tip - tip.position).normalize().dot(-Vec3::Z) > 0.999,
        "it leaves the underside straight down, its normal, towards {under_tip}"
    );
    assert!(
        (tree.root().position - base).normalize().dot(Vec3::Z) > 0.999,
        "and comes onto the block's top straight down too, from {}",
        tree.root().position
    );
    for (child, parent) in tree.struts() {
        let (upper, lower) = (nodes[child].position, nodes[parent].position);
        assert!(
            !leans_too_far(upper, lower),
            "no strut leans past 45 degrees: {upper} to {lower}"
        );
    }
}

#[test]
fn a_small_pillar_is_no_longer_than_thirty_of_its_diameters() {
    // 8 mm from the shelf to the neck: inside the longest run set, but past 30 × 0.2 mm.
    let shelf = box_mesh(Vec3::ZERO, Vec3::splat(10.0));
    let mut fixture = profile();
    let trees = |fixture: &SupportProfile| {
        trees_on(
            &[SupportPoint::new(Vec3::new(5.0, 5.0, 20.0))],
            &shelf,
            Transform::default(),
            Profiles::single(fixture),
        )
    };
    fixture.small_pillar.max_length_mm = 15.0;
    assert!(is_small(&trees(&fixture)[0], &fixture), "0.5 mm across");
    fixture.small_pillar.diameter_mm = 0.2;
    assert!(
        !is_small(&trees(&fixture)[0], &fixture),
        "0.2 mm across, it sways: the regular width holds it"
    );
}

#[test]
fn a_small_pillar_over_a_slope_is_one_straight_strut_meeting_it_nearly_square() {
    // A contact in the air over a ramp z = x / 2, whose face looks up 26.6 degrees off vertical:
    // grown straight down it would bend onto the slope, leant it meets it 3.6 degrees off.
    let model = ramp(10.0, 5.0);
    let fixture = profile();
    let trees = trees_on(
        &[SupportPoint::new(Vec3::new(5.0, 0.0, 8.0))],
        &model,
        Transform::default(),
        Profiles::single(&fixture),
    );
    assert_eq!(trees.len(), 1, "it stands");
    let tree = &trees[0];
    assert!(is_small(tree, &fixture), "built thin, face to face");
    assert!(
        turn_rad(tree) < STRAIGHTER_RAD,
        "one straight strut, turning {} rad",
        turn_rad(tree)
    );
    let up = Vec3::new(-0.5, 0.0, 1.0).normalize();
    let base = tree.landing().base;
    assert!(
        (tree.root().position - base).normalize().dot(up) >= SQUARE_ENOUGH_COS,
        "it comes onto the slope within 20 degrees of its normal, from {}",
        tree.root().position
    );

    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let mesh = mesh_trees(&trees, &placed, Profiles::single(&fixture));
    // Cut askew by at most 20 degrees, the half ball's mark is at most 1 / cos 20 its radius.
    let reach_mm = fixture.small_pillar.radius_mm() / SQUARE_ENOUGH_COS;
    for vertex in mesh.vertices.iter().filter(|v| v.z < v.x / 2.0 - 1e-4) {
        let off = *vertex - base;
        let across = (off - up * off.dot(up)).length();
        assert!(
            across <= reach_mm + 1e-3,
            "inside the slope it is its half ball, {reach_mm} mm round the contact at most; \
             {vertex} is {across} mm off the normal"
        );
    }
}

#[test]
fn two_nodes_in_one_place_on_a_straight_run_are_no_turn() {
    let node = |z: Scalar, parent: Option<usize>, point: Option<usize>| crate::TreeNode {
        position: Vec3::new(0.0, 0.0, z),
        radius_mm: 0.2,
        parent,
        point,
        head: None,
    };
    let landing = Landing {
        base: Vec3::ZERO,
        on_model: false,
        pad_offset: core_geometry::Vec2::ZERO,
        pad_cut: None,
        on_support: false,
    };
    let tree = SupportTree::new(
        vec![
            node(10.0, Some(1), Some(0)),
            node(10.0, Some(2), None),
            node(5.0, None, None),
        ],
        2,
        landing,
    );
    assert!(
        turn_rad(&tree) < 1e-6,
        "straight down, got {}",
        turn_rad(&tree)
    );
}
