use super::*;
use crate::edit::tests::branched_twice;
use crate::edit::{carried, fits};
use crate::fixtures::profile;
use crate::pick::Part;
use core_geometry::{Bvh, Mesh, Transform};

/// The joint of `tree` above the root that two branches meet at.
fn upper_joint(tree: &SupportTree) -> usize {
    let nodes = tree.nodes();
    (0..nodes.len())
        .find(|node| {
            *node != tree.root_index()
                && nodes
                    .iter()
                    .filter(|above| above.parent == Some(*node) && !above.is_leaf())
                    .count()
                    > 1
        })
        .expect("the pair meets at a joint above the root")
}

#[test]
fn a_joint_carried_aside_from_a_merge_at_45_degrees_lowers_the_joint_under_it() {
    let empty = Mesh::default();
    let bvh = Bvh::build(&empty);
    let placed = Placed::new(&empty, &bvh, Transform::default());
    let tree = branched_twice();
    let joint = upper_joint(&tree);
    let step = Vec3::new(STEP_MM, 0.0, 0.0);
    let moved = carried(&tree, &[Part::Node(joint)], step, &placed, &profile())
        .expect("a joint in clear air is carried");
    assert!(
        !allowed(&tree, &moved, &placed, &profile()),
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
    let moved = carried(&tree, &[Part::Node(joint)], step, &placed, &profile())
        .expect("a joint in clear air is carried");

    assert!(eased(&tree, &moved, &placed, &profile()).is_none());
}

/// A step aside that leans the joint's strut past 45 degrees by less than a joint may give, mm.
const STEP_MM: Scalar = 2.0;
/// A step aside no shift within reach makes up for, mm.
const FAR_MM: Scalar = 8.0;
