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

/// Which piece of a support a ray met: a node (joint or tip), the sticks between them (strut,
/// trunk), or the foot where the trunk stands.
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

/// Which part of which support `ray` passes through, nearest first, else the nearest within
/// `slack_mm`. Sticks are capsules, joints wider balls that win ties; slack never beats a hit.
pub fn grab(
    trees: &[SupportTree],
    ray: &Ray,
    profiles: Profiles,
    slack_mm: Scalar,
) -> Option<Grab> {
    grab_along(trees, ray, profiles, slack_mm).map(|(what, _)| what)
}

/// What [`grab`] answers, with how far along `ray` the part was met, millimetres: what
/// a caller compares with whatever else the ray runs into on its way there.
pub fn grab_along(
    trees: &[SupportTree],
    ray: &Ray,
    profiles: Profiles,
    slack_mm: Scalar,
) -> Option<(Grab, Scalar)> {
    let near = |what: Grab, radius: Scalar, from: Vec3, to: Vec3| {
        let (along_ray, distance) = closest_approach(ray, from, to);
        let miss = (distance - radius).max(0.0);
        (miss <= slack_mm.max(0.0)).then_some((what, miss, along_ray, rank(what.part)))
    };
    trees
        .iter()
        .enumerate()
        .flat_map(|(tree, support)| {
            let profile = profiles.of(support.group());
            parts_of(support, profile)
                .map(move |(part, radius, from, to)| (Grab { tree, part }, radius, from, to))
        })
        .filter_map(|(what, radius, from, to)| near(what, radius, from, to))
        .min_by(|a, b| {
            a.1.total_cmp(&b.1)
                .then_with(|| a.2.total_cmp(&b.2))
                .then_with(|| a.3.cmp(&b.3))
        })
        .map(|(what, _, along_ray, _)| (what, along_ray))
}

/// Which support `ray` passes through or within `slack_mm` of, as the index of its `SupportPoint`;
/// a shared trunk alone hits nothing, as no one support could be taken away.
pub fn support_under(
    trees: &[SupportTree],
    ray: &Ray,
    profiles: Profiles,
    slack_mm: Scalar,
) -> Option<usize> {
    let hit = grab(trees, ray, profiles, slack_mm)?;
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

/// Distance along `ray` of its closest approach to the segment `from`..`to` and how far apart
/// they are there, mm; the clamped closest-points solve, ray forward only.
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
mod tests;
