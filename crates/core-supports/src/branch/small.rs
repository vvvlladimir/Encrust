use crate::Landing;
use crate::column::{
    LAST_RESORT_SLOPE_DEG, MAX_LEAN_PER_DROP, PLATE_Z_MM, SURFACE_EPSILON_MM, face_normal,
};
use crate::group::Profiles;
use crate::mesher::{ground_normal, is_small, longest_run_mm, run_mm, sound};
use crate::placed::Placed;
use crate::tree::{SupportTree, TreeNode};
use core_geometry::{Ray, Scalar, Vec2, Vec3};
use printer_profiles::SupportProfile;
use rayon::prelude::*;

/// How far off its face's normal a small pillar may meet that face unbent, as the cosine: 20
/// degrees, where its mark is 6 % longer one way than round, under a pixel at 0.24 mm (ADR 0348).
const SQUARE_ENOUGH_COS: Scalar = 0.94;

/// The leans off straight down a small pillar looks along for a face, degrees, each
/// [`AROUND`] ways round: out to the 45 degrees every strut is held to (ADR 0249).
const LEANS_DEG: [Scalar; 3] = [15.0, 30.0, 45.0];

const AROUND: u32 = 16;

/// How far a small pillar runs out along the normal of each face it meets before it bends,
/// mm: one diameter, so the bend's dome keeps a radius off the face (ADR 0348).
fn square_mm(profile: &SupportProfile) -> Scalar {
    profile.small_pillar.diameter_mm
}

/// The straightest sound small pillar holding `contact` for `point`, the shortest of those: square
/// off its face, then down within 45 degrees onto a face looking back at it (ADR 0348).
pub(super) fn small_pillar(
    contact: Vec3,
    point: usize,
    group: u16,
    placed: &Placed,
    profiles: Profiles,
) -> Option<SupportTree> {
    let profile = profiles.of(group);
    if !profile.small_pillar.enabled || !profile.land_on_model {
        return None;
    }
    let normal = face_normal(placed, contact);
    let from = normal.map_or(contact, |n| contact + n * square_mm(profile));
    // Along the normal first: onto a face across from it, both ends are square with no bend.
    let square = normal.filter(|n| !leans_too_far(Vec3::ZERO, *n));
    let mut found: Vec<((Scalar, Scalar), SupportTree)> = square
        .into_iter()
        .chain(directions())
        .filter_map(|direction| {
            let base = landing_along(placed, from, direction, profile)?;
            let tip = TreeNode {
                position: contact,
                radius_mm: profile.small_pillar.radius_mm(),
                parent: None,
                point: Some(point),
                head: None,
            };
            let mut tree = SupportTree::new(vec![tip], 0, on_part(base));
            tree.set_group(group);
            let tree = squared(&tree, placed, profile)?;
            is_small(&tree, profile).then(|| ((turn_rad(&tree), run_mm(&tree)), tree))
        })
        .collect();
    found.sort_by(|(a, _), (b, _)| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    found
        .into_iter()
        .map(|(_, tree)| tree)
        .find(|tree| sound(tree, profiles, placed))
}

/// How much a one-tip tree turns on its way from the contact to where it stands: the sum of
/// the angles at its joints, radians; a straight strut turns none.
fn turn_rad(tree: &SupportTree) -> Scalar {
    let nodes = tree.nodes();
    let mut path = Vec::with_capacity(nodes.len() + 1);
    let mut at = nodes.iter().position(TreeNode::is_leaf);
    // No longer than the tree is deep; the count guards a tree with a loop in it.
    for _ in 0..nodes.len() {
        let Some(here) = at else {
            break;
        };
        path.push(nodes[here].position);
        at = nodes[here].parent;
    }
    path.push(tree.landing().base);
    path.windows(3)
        .map(|three| {
            let (into, out) = (three[1] - three[0], three[2] - three[1]);
            into.normalize_or_zero()
                .dot(out.normalize_or_zero())
                .clamp(-1.0, 1.0)
                .acos()
        })
        .sum()
}

/// Every small pillar of `forest` standing on the part, made the straighter of what it grew
/// into and what [`small_pillar`] finds, then squared off both its faces where that is sound.
pub(super) fn tidy_small_pillars(forest: &mut [SupportTree], placed: &Placed, profiles: Profiles) {
    forest.par_iter_mut().for_each(|tree| {
        let profile = profiles.of(tree.group());
        let landing = tree.landing();
        if !landing.on_model || landing.on_support || !is_small(tree, profile) {
            return;
        }
        let squared = squared(tree, placed, profile)
            .filter(|squared| squared == tree || sound(squared, profiles, placed))
            .unwrap_or_else(|| tree.clone());
        let tip = tree.nodes().iter().find(|node| node.is_leaf());
        let straighter = tip
            .and_then(|tip| {
                let point = tip.point?;
                let group = tip.head.unwrap_or(tree.group());
                small_pillar(tip.position, point, group, placed, profiles)
            })
            .filter(|found| turn_rad(found) + STRAIGHTER_RAD < turn_rad(&squared));
        *tree = straighter.unwrap_or(squared);
    });
}

/// How much less a small pillar must turn to replace the one a support grew into, radians:
/// about five degrees, so a run is not rebuilt for a turn nobody sees.
const STRAIGHTER_RAD: Scalar = 0.09;

/// `tree` with a small pillar's ends leaving their faces along the normal for [`square_mm`],
/// so only the half ball meets each; `None` where a bend would lean a strut past 45 degrees.
fn squared(tree: &SupportTree, placed: &Placed, profile: &SupportProfile) -> Option<SupportTree> {
    let landing = tree.landing();
    if !landing.on_model || landing.on_support || !is_small(tree, profile) {
        return Some(tree.clone());
    }
    let mut squared = tree.clone();
    let reach_mm = square_mm(profile);
    let radius_mm = profile.small_pillar.radius_mm();
    let tip = squared.nodes().iter().position(TreeNode::is_leaf)?;
    if let Some(normal) = face_normal(placed, squared.nodes()[tip].position) {
        let contact = squared.nodes()[tip].position;
        let next = squared.nodes()[tip]
            .parent
            .map_or(landing.base, |parent| squared.nodes()[parent].position);
        if !same_way(next - contact, normal) {
            let knee = contact + normal * reach_mm;
            if leans_too_far(knee, next) {
                return None;
            }
            squared.bend_under(tip, knee, radius_mm);
        }
    }
    if let Some(normal) = ground_normal(placed, landing) {
        let root = squared.root().position;
        if !same_way(root - landing.base, normal) {
            let knee = landing.base + normal * reach_mm;
            if leans_too_far(root, knee) {
                return None;
            }
            squared.stand_on_knee(knee, radius_mm);
        }
    }
    Some(squared)
}

/// Where a small pillar leaving `from` along `direction` stands: a face of the part looking back
/// at it, no steeper than a last resort, not painted out, within the longest run.
fn landing_along(
    placed: &Placed,
    from: Vec3,
    direction: Vec3,
    profile: &SupportProfile,
) -> Option<Vec3> {
    let ray = Ray::new(from + direction * SURFACE_EPSILON_MM, direction);
    let hit = placed.raycast(&ray)?;
    let steepest_cos = LAST_RESORT_SLOPE_DEG.to_radians().cos();
    let stands = hit.distance <= longest_run_mm(profile)
        && hit.normal.dot(-direction) > 0.0
        && hit.normal.z >= steepest_cos
        && hit.point.z > PLATE_Z_MM
        && !placed.blocks_at(hit.point);
    stands.then_some(hit.point)
}

/// Straight down, then [`AROUND`] ways round at each of [`LEANS_DEG`].
fn directions() -> impl Iterator<Item = Vec3> {
    let leaning = LEANS_DEG.into_iter().flat_map(|lean_deg| {
        let lean = lean_deg.to_radians();
        (0..AROUND).map(move |step| {
            let turn = std::f32::consts::TAU * step as Scalar / AROUND as Scalar;
            Vec3::new(
                lean.sin() * turn.cos(),
                lean.sin() * turn.sin(),
                -lean.cos(),
            )
        })
    });
    std::iter::once(-Vec3::Z).chain(leaning)
}

fn on_part(base: Vec3) -> Landing {
    Landing {
        base,
        on_model: true,
        pad_offset: Vec2::ZERO,
        pad_cut: None,
        on_support: false,
    }
}

fn same_way(along: Vec3, normal: Vec3) -> bool {
    along.normalize_or_zero().dot(normal) >= SQUARE_ENOUGH_COS
}

/// Whether a strut from `upper` down to `lower` leans past 45 degrees, or does not go down.
fn leans_too_far(upper: Vec3, lower: Vec3) -> bool {
    let drop_mm = upper.z - lower.z;
    drop_mm <= 0.0 || (upper - lower).truncate().length() > drop_mm * MAX_LEAN_PER_DROP
}

#[cfg(test)]
mod tests;
