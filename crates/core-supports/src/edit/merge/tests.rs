use super::*;
use crate::fixtures::{box_mesh, profile, trees_on};
use crate::{Profiles, SupportPoint};
use core_geometry::{Bvh, Mesh, Transform};

/// A slab whose underside at z = 20 runs over x 0..30, y −5..5.
fn slab() -> Mesh {
    box_mesh(Vec3::new(0.0, -5.0, 20.0), Vec3::new(30.0, 5.0, 22.0))
}

/// One support grown alone under the slab at each of `xs`, so none of them share a trunk.
fn alone_under(model: &Mesh, xs: &[Scalar], profile: &SupportProfile) -> Vec<SupportTree> {
    xs.iter()
        .map(|x| {
            trees_on(
                &[SupportPoint::new(Vec3::new(*x, 0.0, 20.0))],
                model,
                Transform::default(),
                Profiles::single(profile),
            )
            .pop()
            .expect("a contact under the slab in clear air stands")
        })
        .collect()
}

#[test]
fn two_supports_merge_into_one_trunk_from_where_their_necks_meet_at_45_degrees() {
    let model = slab();
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let profile = profile();
    let trees = alone_under(&model, &[5.0, 15.0], &profile);
    let profiles = Profiles::single(&profile);

    let tree = merged(&trees, &[0, 1], &placed, profiles).expect("clear air between them");

    assert_eq!(tree.tip_count(), 2, "both tips are still held");
    let neck = trees[0].root().position;
    // Two necks 10 mm apart at one height meet midway, 5 mm under them at 45 degrees.
    let joint = tree.root().position;
    assert!(
        (joint.x - 10.0).abs() < 1e-3 && (joint.z - (neck.z - 5.0)).abs() < 1e-3,
        "the joint stands at x = 10, z = {}, got {joint}",
        neck.z - 5.0
    );
    assert!(
        !tree.landing().on_model
            && (tree.landing().base.truncate() - joint.truncate()).length() < 1e-3,
        "one trunk stands straight under the joint on the plate"
    );
    assert!(fits(&tree, &placed, &profile));
}

#[test]
fn three_supports_merge_into_one_tree_holding_all_their_tips() {
    let model = slab();
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let trees = alone_under(&model, &[5.0, 12.0, 20.0], &profile());

    let tree = merged(&trees, &[2, 0, 1], &placed, Profiles::single(&profile()))
        .expect("clear air between them");

    assert_eq!(tree.tip_count(), 3);
    let roots = tree
        .nodes()
        .iter()
        .filter(|node| node.parent.is_none())
        .count();
    assert_eq!(roots, 1, "every node reaches one root");
}

#[test]
fn a_wall_between_two_supports_refuses_the_merge() {
    let mut model = slab();
    model.append(&box_mesh(
        Vec3::new(9.5, -5.0, 0.0),
        Vec3::new(10.5, 5.0, 20.0),
    ));
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let trees = alone_under(&model, &[5.0, 15.0], &profile());

    let refused = merged(&trees, &[0, 1], &placed, Profiles::single(&profile()));

    assert_eq!(refused, Err(Refused::Collides));
}

#[test]
fn a_joint_over_a_block_it_may_not_stand_on_is_refused() {
    let mut profile = profile();
    profile.land_on_model = false;
    let mut model = slab();
    model.append(&box_mesh(
        Vec3::new(8.0, -5.0, 0.0),
        Vec3::new(12.0, 5.0, 8.0),
    ));
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let trees = alone_under(&model, &[5.0, 15.0], &profile);

    let refused = merged(&trees, &[0, 1], &placed, Profiles::single(&profile));

    assert_eq!(refused, Err(Refused::NoFooting));
}

#[test]
fn one_support_alone_has_nothing_to_merge_with() {
    let model = slab();
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let profile = profile();
    let trees = alone_under(&model, &[5.0, 15.0], &profile);
    let profiles = Profiles::single(&profile);

    assert_eq!(
        merged(&trees, &[1, 1], &placed, profiles),
        Err(Refused::NoHold)
    );
    assert_eq!(
        merged(&trees, &[0, 7], &placed, profiles),
        Err(Refused::NoHold)
    );
}
