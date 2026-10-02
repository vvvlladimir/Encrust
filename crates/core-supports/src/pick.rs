use core_geometry::{Ray, Scalar, Vec3};
use printer_profiles::SupportProfile;

use crate::SupportTree;
use crate::group::Profiles;

/// Below this the ray runs parallel to the strut's axis and the two have no single
/// closest pair; the ray's own origin is then as good a place to measure from as any.
const PARALLEL_EPSILON: Scalar = 1e-9;

/// How much wider than the pillar a joint is aimed at. A ball the size of the stick it
/// sits on could never be hit first, and a joint is what the cursor is usually after.
const JOINT_REACH: Scalar = 1.6;

/// Which piece of a support a ray met: the parts a hand takes hold of one at a time.
///
/// A node is a joint or a tip — carrying one bends whatever meets there. A strut and the
/// trunk are the sticks between them, and the foot is where the trunk stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    /// A joint or a tip, by its place in the tree's nodes.
    Node(usize),
    /// The stick hanging under a node, named by that node.
    Strut(usize),
    /// The stick under the root, down to the foot.
    Trunk,
    /// Where the trunk stands.
    Foot,
}

/// What a ray met, and on which of the trees it was given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grab {
    pub tree: usize,
    pub part: Part,
}

/// Which part of which support `ray` passes through, nearest the ray's origin first.
///
/// Sticks are aimed at as capsules of the pillar's radius and the joints between them as
/// balls a little wider, and a ball wins a tie with the stick it sits on: a joint is the
/// smaller target and the one that was meant when both are under the cursor.
pub fn grab(trees: &[SupportTree], ray: &Ray, profiles: Profiles) -> Option<Grab> {
    trees
        .iter()
        .enumerate()
        .flat_map(|(tree, support)| {
            let profile = profiles.of(support.group());
            parts_of(support, profile)
                .map(move |(part, radius, from, to)| (Grab { tree, part }, radius, from, to))
        })
        .filter_map(|(what, radius, from, to)| {
            let (along_ray, distance) = closest_approach(ray, from, to);
            (distance <= radius).then_some((what, along_ray, rank(what.part)))
        })
        .min_by(|(_, a, a_rank), (_, b, b_rank)| a.total_cmp(b).then_with(|| a_rank.cmp(b_rank)))
        .map(|(what, _, _)| what)
}

/// Which support `ray` passes through, as the index of the `SupportPoint` it holds.
/// A ray that meets only a shared trunk hits nothing, because there is no one support to
/// take away.
pub fn support_under(trees: &[SupportTree], ray: &Ray, profiles: Profiles) -> Option<usize> {
    let hit = grab(trees, ray, profiles)?;
    let tree = trees.get(hit.tree)?;
    match hit.part {
        Part::Node(node) => tree
            .nodes()
            .get(node)
            .and_then(|node| node.point)
            .or_else(|| lone_point(tree)),
        Part::Strut(node) => tree
            .nodes()
            .get(node)
            .and_then(|node| node.point)
            .or_else(|| lone_point(tree)),
        Part::Trunk | Part::Foot => lone_point(tree),
    }
}

/// The one support a tree carries, or `None` when it carries several and no single part
/// of its trunk belongs to any of them.
fn lone_point(tree: &SupportTree) -> Option<usize> {
    (tree.tip_count() == 1)
        .then(|| tree.points().next())
        .flatten()
}

/// Which of two parts at the same distance is the one that was meant: the smaller target.
fn rank(part: Part) -> u8 {
    match part {
        Part::Node(_) | Part::Foot => 0,
        Part::Strut(_) | Part::Trunk => 1,
    }
}

/// Every part of `tree` as the segment it is aimed at and the radius it is aimed with.
/// A joint is a ball, so its segment has no length.
fn parts_of<'a>(
    tree: &'a SupportTree,
    profile: &'a SupportProfile,
) -> impl Iterator<Item = (Part, Scalar, Vec3, Vec3)> + 'a {
    let radius = profile.pillar_radius_mm();
    let nodes = tree.nodes().iter().enumerate().map(move |(index, node)| {
        (
            Part::Node(index),
            radius * JOINT_REACH,
            node.position,
            node.position,
        )
    });
    let struts = tree.struts().map(move |(child, parent)| {
        (
            Part::Strut(child),
            radius,
            tree.nodes()[child].position,
            tree.nodes()[parent].position,
        )
    });

    let (top, bottom) = trunk_of(tree, profile);
    let trunk = std::iter::once((Part::Trunk, radius, top, bottom));
    let foot = std::iter::once((
        Part::Foot,
        profile.base_radius_mm().max(radius * JOINT_REACH),
        tree.landing().base,
        tree.landing().base,
    ));

    nodes.chain(struts).chain(trunk).chain(foot)
}

/// The trunk of `tree`: from the node everything hangs off down to the top of its foot.
fn trunk_of(tree: &SupportTree, profile: &SupportProfile) -> (Vec3, Vec3) {
    let root = tree.root().position;
    let landing = tree.landing();
    (
        root,
        Vec3::new(
            landing.base.x,
            landing.base.y,
            landing.pillar_bottom_z(profile),
        ),
    )
}

/// Distance along `ray` of its closest approach to the segment `from`..`to`, and how far
/// apart the two are there. Both in millimetres.
///
/// The standard clamped solve for the closest pair of points on two lines, with the ray
/// clamped to its forward half and the segment to its ends.
fn closest_approach(ray: &Ray, from: Vec3, to: Vec3) -> (Scalar, Scalar) {
    let segment = to - from;
    let offset = ray.origin - from;
    let segment_length_sq = segment.length_squared();
    let along_segment_offset = segment.dot(offset);
    let along_ray_offset = ray.direction.dot(offset);

    if segment_length_sq < PARALLEL_EPSILON {
        let along_ray = (-along_ray_offset).max(0.0);
        return (along_ray, (ray.at(along_ray) - from).length());
    }

    let projection = ray.direction.dot(segment);
    let denominator = segment_length_sq - projection * projection;

    let mut along_ray = if denominator > PARALLEL_EPSILON {
        (projection * along_segment_offset - along_ray_offset * segment_length_sq) / denominator
    } else {
        0.0
    };
    along_ray = along_ray.max(0.0);

    let on_segment =
        ((projection * along_ray + along_segment_offset) / segment_length_sq).clamp(0.0, 1.0);
    along_ray = (on_segment * projection - along_ray_offset).max(0.0);

    let distance = (ray.at(along_ray) - (from + segment * on_segment)).length();
    (along_ray, distance)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SupportPoint;
    use crate::tests::{profile, trees_on};
    use core_geometry::{Mesh, Transform};

    /// Supports standing on the plate at each of `positions`, all 20 mm tall.
    fn standing(positions: &[(Scalar, Scalar)], profile: &SupportProfile) -> Vec<SupportTree> {
        let points: Vec<SupportPoint> = positions
            .iter()
            .map(|(x, y)| SupportPoint::new(Vec3::new(*x, *y, 20.0)))
            .collect();
        trees_on(
            &points,
            &Mesh::default(),
            Transform::default(),
            Profiles::single(profile),
        )
    }

    #[test]
    fn a_ray_through_the_pillar_finds_its_support() {
        let profile = profile();
        let trees = standing(&[(10.0, 10.0)], &profile);
        let ray = Ray::new(Vec3::new(-50.0, 10.0, 10.0), Vec3::X);
        assert_eq!(
            support_under(&trees, &ray, Profiles::single(&profile)),
            Some(0)
        );
    }

    #[test]
    fn a_ray_through_a_shared_trunk_takes_hold_of_the_trunk() {
        let profile = profile();
        // Close enough to merge, so the pair hangs off one trunk.
        let trees = standing(&[(10.0, 10.0), (13.0, 10.0)], &profile);
        assert_eq!(trees.len(), 1, "the two tips share a trunk");

        // Aimed along x halfway up: under both tips, over the foot, through the trunk
        // they meet on.
        let ray = Ray::new(Vec3::new(-50.0, 10.0, 8.0), Vec3::X);
        assert_eq!(
            grab(&trees, &ray, Profiles::single(&profile)),
            Some(Grab {
                tree: 0,
                part: Part::Trunk
            })
        );
        assert_eq!(
            support_under(&trees, &ray, Profiles::single(&profile)),
            None,
            "a shared trunk belongs to no single support"
        );
    }

    #[test]
    fn a_ray_through_the_strut_under_a_tip_takes_hold_of_that_strut() {
        let profile = profile();
        let trees = standing(&[(10.0, 10.0), (13.0, 10.0)], &profile);
        // Just under the tips, where each still hangs on a strut of its own.
        let ray = Ray::new(Vec3::new(-50.0, 10.0, 19.0), Vec3::X);
        let held = grab(&trees, &ray, Profiles::single(&profile)).expect("the strut is there");

        assert_eq!(held.tree, 0);
        assert!(
            matches!(held.part, Part::Strut(_)),
            "a tip hangs on a strut of its own until the joint under it, got {:?}",
            held.part
        );
    }

    #[test]
    fn a_ray_at_the_plate_takes_hold_of_the_foot() {
        let profile = profile();
        let trees = standing(&[(10.0, 10.0)], &profile);
        let ray = Ray::new(Vec3::new(-50.0, 10.0, 0.2), Vec3::X);
        assert_eq!(
            grab(&trees, &ray, Profiles::single(&profile)),
            Some(Grab {
                tree: 0,
                part: Part::Foot
            })
        );
    }

    #[test]
    fn a_ray_beside_the_pillar_finds_nothing() {
        let profile = profile();
        let trees = standing(&[(10.0, 10.0)], &profile);
        // Two millimetres to the side of a pillar one millimetre in radius.
        let ray = Ray::new(Vec3::new(-50.0, 12.0, 10.0), Vec3::X);
        assert_eq!(
            support_under(&trees, &ray, Profiles::single(&profile)),
            None
        );
    }

    #[test]
    fn the_nearer_of_two_supports_in_line_wins() {
        let profile = profile();
        let trees = standing(&[(50.0, 10.0), (10.0, 10.0)], &profile);
        let ray = Ray::new(Vec3::new(-50.0, 10.0, 10.0), Vec3::X);
        assert_eq!(
            support_under(&trees, &ray, Profiles::single(&profile)),
            Some(1),
            "the support at x = 10 is met first"
        );
    }

    #[test]
    fn a_ray_pointing_away_from_the_support_finds_nothing() {
        let profile = profile();
        let trees = standing(&[(10.0, 10.0)], &profile);
        let ray = Ray::new(Vec3::new(-50.0, 10.0, 10.0), -Vec3::X);
        assert_eq!(
            support_under(&trees, &ray, Profiles::single(&profile)),
            None
        );
    }

    #[test]
    fn a_ray_past_the_top_of_the_support_finds_nothing() {
        let profile = profile();
        let trees = standing(&[(10.0, 10.0)], &profile);
        // The contact is at z = 20; this passes 5 mm over it.
        let ray = Ray::new(Vec3::new(-50.0, 10.0, 25.0), Vec3::X);
        assert_eq!(
            support_under(&trees, &ray, Profiles::single(&profile)),
            None
        );
    }

    #[test]
    fn a_ray_down_the_supports_own_axis_still_hits_it() {
        let profile = profile();
        let trees = standing(&[(10.0, 10.0)], &profile);
        let ray = Ray::new(Vec3::new(10.0, 10.0, 60.0), -Vec3::Z);
        assert_eq!(
            support_under(&trees, &ray, Profiles::single(&profile)),
            Some(0)
        );
    }

    #[test]
    fn a_branch_is_picked_by_the_tip_it_carries() {
        let profile = profile();
        let trees = standing(&[(10.0, 10.0), (14.0, 10.0)], &profile);
        assert_eq!(trees.len(), 1, "the pair merges into one tree");

        // Just under the second tip, where only its own branch runs.
        let ray = Ray::new(Vec3::new(14.0, -50.0, 19.5), Vec3::Y);
        assert_eq!(
            support_under(&trees, &ray, Profiles::single(&profile)),
            Some(1)
        );
    }

    #[test]
    fn a_ray_through_a_shared_trunk_takes_nothing_away() {
        let profile = profile();
        let trees = standing(&[(10.0, 10.0), (14.0, 10.0)], &profile);
        let trunk_z = trees[0].root().position.z;

        // A millimetre under the joint the two tips share, where no one tip owns the
        // material.
        let ray = Ray::new(Vec3::new(12.0, -50.0, trunk_z - 1.0), Vec3::Y);
        assert_eq!(
            support_under(&trees, &ray, Profiles::single(&profile)),
            None
        );
    }

    #[test]
    fn nothing_is_under_a_ray_when_there_are_no_supports() {
        let ray = Ray::new(Vec3::ZERO, Vec3::X);
        assert_eq!(support_under(&[], &ray, Profiles::single(&profile())), None);
    }
}
