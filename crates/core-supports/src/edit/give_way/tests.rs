use super::*;
use crate::edit::tests::{branched_twice, upper_joint};
use crate::edit::{carried, fits};
use crate::fixtures::{box_mesh, profile};
use crate::pick::Part;
use core_geometry::{Bvh, Mesh, Transform};

#[test]
fn a_joint_carried_aside_from_a_merge_at_45_degrees_lowers_the_joint_under_it() {
    let empty = Mesh::default();
    let bvh = Bvh::build(&empty);
    let placed = Placed::new(&empty, &bvh, Transform::default());
    let tree = branched_twice();
    let joint = upper_joint(&tree);
    let step = Vec3::new(STEP_MM, 0.0, 0.0);
    let moved = carried(
        &tree,
        tree.landing(),
        &[Part::Node(joint)],
        step,
        &placed,
        &profile(),
    )
    .expect("a joint in clear air is carried");
    assert!(
        allowed(&tree, &moved, &placed, &profile()) == Err(Refused::Leans),
        "the strut to the root, at 45 degrees before, now leans further"
    );

    let eased = eased(&tree, &moved, &placed, &profile()).expect("the root may give way");

    let (was, now) = (tree.root().position, eased.root().position);
    assert!(
        now.z < was.z && (now - was).length() <= MOST_GIVE_MM + 1e-4,
        "the root went down by at most {MOST_GIVE_MM} mm, from {was} to {now}"
    );
    assert_eq!(
        eased.nodes()[joint].position,
        moved.nodes()[joint].position,
        "the joint stays where the hand put it"
    );
    assert!(fits(&eased, &placed, &profile()));
}

#[test]
fn a_joint_carried_further_than_its_neighbours_can_follow_is_refused() {
    let empty = Mesh::default();
    let bvh = Bvh::build(&empty);
    let placed = Placed::new(&empty, &bvh, Transform::default());
    let tree = branched_twice();
    let joint = upper_joint(&tree);
    let step = Vec3::new(FAR_MM, 0.0, 0.0);
    let moved = carried(
        &tree,
        tree.landing(),
        &[Part::Node(joint)],
        step,
        &placed,
        &profile(),
    )
    .expect("a joint in clear air is carried");

    assert_eq!(
        eased(&tree, &moved, &placed, &profile()).err(),
        Some(Refused::Leans)
    );
}

#[test]
fn a_neck_never_gives_way_since_its_head_would_turn_off_its_face() {
    let tree = branched_twice();
    let joint = upper_joint(&tree);
    let neck = (0..tree.nodes().len())
        .find(|node| {
            tree.nodes()[*node].parent == Some(joint)
                && tree
                    .struts()
                    .any(|(tip, under)| under == *node && tree.nodes()[tip].is_leaf())
        })
        .expect("a neck hangs from the upper joint");
    let step = Vec3::new(0.0, -2.0, 0.0);
    let moved = carried(
        &tree,
        tree.landing(),
        &[Part::Node(joint)],
        step,
        &Placed::new(
            &Mesh::default(),
            &Bvh::build(&Mesh::default()),
            Transform::default(),
        ),
        &profile(),
    )
    .expect("a joint in clear air is carried");
    // A block in the way of the strut from that neck down to where the joint went.
    let middle = tree.nodes()[neck]
        .position
        .lerp(moved.nodes()[joint].position, 0.5);
    let block = box_mesh(middle - Vec3::splat(0.2), middle + Vec3::splat(0.2));
    let bvh = Bvh::build(&block);
    let placed = Placed::new(&block, &bvh, Transform::default());

    assert!(
        eased(&tree, &moved, &placed, &profile()).is_err(),
        "only a joint no head hangs from moves aside, so nothing makes room"
    );
}

/// A step aside that leans the joint's strut past 45 degrees by less than a joint may give, mm.
const STEP_MM: Scalar = 2.0;
/// A step aside no shift within reach makes up for, mm.
const FAR_MM: Scalar = 8.0;
