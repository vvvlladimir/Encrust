use super::*;
use crate::edit::standing_on;
use crate::edit::tests::{branched_twice, upper_joint};
use crate::fixtures::{profile, trees_on};
use crate::{Landing, SupportPoint};
use core_geometry::{Bvh, Mesh, Transform, Vec3};

/// `host` and a column in clear air beside it standing on its trunk, `share` of the way down.
fn with_rider(host: &SupportTree, share: f32) -> Vec<SupportTree> {
    let mut rider = trees_on(
        &[SupportPoint::new(Vec3::new(25.0, 10.0, 20.0))],
        &Mesh::default(),
        Transform::default(),
        Profiles::single(&profile()),
    )
    .pop()
    .expect("a column in clear air stands");
    let bottom = host
        .landing()
        .base
        .with_z(host.landing().pillar_bottom_z(&profile()));
    rider.stand_on(Landing {
        on_model: true,
        on_support: true,
        ..Landing::on_part(host.root().position.lerp(bottom, share))
    });
    vec![host.clone(), rider]
}

#[test]
fn a_support_on_one_whose_joints_give_way_is_seated_on_it_as_it_ends_up() {
    let empty = Mesh::default();
    let bvh = Bvh::build(&empty);
    let placed = Placed::new(&empty, &bvh, Transform::default());
    let host = branched_twice();
    // Seated just under the root, which the carry below lowers past it.
    let standing = with_rider(&host, 0.05);
    let profile = profile();
    let profiles = Profiles::single(&profile);
    assert_eq!(standing_on(&standing, 0, profiles), vec![1]);
    let joint = upper_joint(&host);

    let moved = carried_held(
        &standing,
        &[],
        &[(0, vec![Part::Node(joint)])],
        Vec3::X * 2.0,
        &placed,
        profiles,
    )
    .expect("a joint in clear air is carried");

    assert_eq!(moved.refused, None);
    let trees: Vec<SupportTree> = moved.moved.into_iter().map(|(_, tree)| tree).collect();
    assert!(
        trees[0].root().position.z < host.root().position.z,
        "the root gave way downwards"
    );
    assert_eq!(
        standing_on(&trees, 0, profiles),
        vec![1],
        "the rider stands on the host's trunk as it ends up"
    );
}

#[test]
fn a_tree_held_and_standing_on_another_held_is_carried_once_as_held() {
    let empty = Mesh::default();
    let bvh = Bvh::build(&empty);
    let placed = Placed::new(&empty, &bvh, Transform::default());
    let standing = with_rider(&branched_twice(), 0.5);
    let profile = profile();
    let profiles = Profiles::single(&profile);

    let moved = carried_held(
        &standing,
        &[],
        &[(0, vec![Part::Trunk]), (1, Vec::new())],
        Vec3::X * 2.0,
        &placed,
        profiles,
    )
    .expect("both stand in clear air");

    let indices: Vec<usize> = moved.moved.iter().map(|(index, _)| *index).collect();
    assert_eq!(indices, vec![0, 1], "each tree once, held first");
    assert_eq!(
        moved.moved[1].1.landing(),
        standing[1].landing(),
        "the rider held still is not seated again on the host"
    );
}

#[test]
fn a_support_put_down_by_hand_takes_the_one_standing_on_it_along() {
    let empty = Mesh::default();
    let bvh = Bvh::build(&empty);
    let placed = Placed::new(&empty, &bvh, Transform::default());
    let standing = with_rider(&branched_twice(), 0.5);
    let profile = profile();
    let profiles = Profiles::single(&profile);
    let host = &standing[0];
    let put = carried(
        host,
        host.landing(),
        &[Part::Trunk],
        Vec3::X * 2.0,
        &placed,
        &profile,
    )
    .expect("a column in clear air is carried");

    let moved =
        with_riders(&standing, 0, &put, &placed, profiles).expect("the rider keeps its seat");

    assert_eq!(moved.refused, None);
    assert_eq!(
        moved.moved[0],
        (0, put),
        "the host as the hand put it, first"
    );
    let trees: Vec<SupportTree> = moved.moved.into_iter().map(|(_, tree)| tree).collect();
    assert_eq!(
        standing_on(&trees, 0, profiles),
        vec![1],
        "the rider stands on the host where it was put"
    );
}
