use crate::placed::Placed;
use core_geometry::{Buckets, Mesh, Scalar, Vec2, Vec3, convex_hull};
use printer_profiles::{BracePattern, Bracing, SupportProfile};
use rayon::prelude::*;

use crate::clear::{Beam, clear};
use crate::group::Profiles;
use crate::mesher::{trunk_axis_at, trunk_radius_at_mm};
use crate::tree::SupportTree;

/// Two feet closer together than this stand in the same place, and the hull cannot tell
/// them apart. Well under one pixel of any panel.
pub(crate) const SAME_PLACE_MM: Scalar = 1e-4;

/// How long a skate is, and how wide, as multiples of the foot's own radius. A skate
/// grips along its length and releases across its width, so the two are not the same.
pub const SKATE_LENGTH: Scalar = 1.6;
pub const SKATE_WIDTH: Scalar = 0.55;

/// How far in the bevel pulls the skate's footprint, as a fraction of its size; the flat pad
/// meets the plate on a rim a blade gets under (ADR 0044).
pub const SKATE_BEVEL: Scalar = 0.3;

/// Half the width of a skate across its narrow way, millimetres, which is the most a
/// flare standing in one may spread to.
pub(crate) fn skate_half_width_mm(profile: &SupportProfile) -> Scalar {
    profile.base_radius_mm() * SKATE_WIDTH * (1.0 - SKATE_BEVEL)
}

/// One skate foot: a pad lying flat on the plate, bevelled all round; `toe` is the horizontal
/// direction it is elongated along, away from the part it came down from (ADR 0043).
pub(crate) fn skate(mesh: &mut Mesh, base: Vec3, toe: Vec3, profile: &SupportProfile) {
    let thickness = profile.bottom.platform_thickness_mm;
    let radius = profile.base_radius_mm();
    let along = toe * (radius * SKATE_LENGTH);
    let across = Vec3::new(-toe.y, toe.x, 0.0) * (radius * SKATE_WIDTH);
    let up = Vec3::Z * thickness;
    let inset = 1.0 - SKATE_BEVEL;

    // The footprint on the plate is the bevelled one; the full size is at the top.
    let first = mesh.vertices.len() as u32;
    for corner in [
        base - along * inset - across * inset,
        base - along * inset + across * inset,
        base + along * inset + across * inset,
        base + along * inset - across * inset,
        base - along + up - across,
        base - along + up + across,
        base + along + up + across,
        base + along + up - across,
    ] {
        mesh.vertices.push(corner);
    }

    // The bottom quad wound clockwise seen from above, so its normal points down.
    for face in [
        [0, 1, 2],
        [0, 2, 3],
        [4, 6, 5],
        [4, 7, 6],
        [0, 4, 5],
        [0, 5, 1],
        [1, 5, 6],
        [1, 6, 2],
        [2, 6, 7],
        [2, 7, 3],
        [3, 7, 4],
        [3, 4, 0],
    ] {
        mesh.faces
            .push([first + face[0], first + face[1], first + face[2]]);
    }
}

/// Which way the toe of `tree`'s skate points: away from the part it came down from, or the
/// plate's x for a support that never leaned.
pub(crate) fn toe_of(tree: &SupportTree) -> Vec3 {
    let root = tree.root().position;
    let highest = tree
        .nodes()
        .iter()
        .filter(|node| node.is_leaf())
        .max_by(|a, b| a.position.z.total_cmp(&b.position.z));

    let away = highest.map_or(Vec2::ZERO, |tip| {
        Vec2::new(root.x - tip.position.x, root.y - tip.position.y)
    });
    if away.length() < SAME_PLACE_MM {
        return Vec3::X;
    }
    let away = away.normalize();
    Vec3::new(away.x, away.y, 0.0)
}

/// The convex hull of `points`, counter-clockwise, and their bounding rectangle where they
/// lie on one line or in one place.
pub(crate) fn hull(points: &[Vec2]) -> Vec<Vec2> {
    let chain = convex_hull(points);
    if chain.len() < 3 {
        rectangle(points)
    } else {
        chain
    }
}

/// The bounding rectangle of `points`, counter-clockwise.
fn rectangle(points: &[Vec2]) -> Vec<Vec2> {
    let mut low = Vec2::splat(Scalar::INFINITY);
    let mut high = Vec2::splat(Scalar::NEG_INFINITY);
    for &point in points {
        low = low.min(point);
        high = high.max(point);
    }
    if !low.x.is_finite() {
        return Vec::new();
    }

    // A single foot has no rectangle of its own, so it gets one the margin will widen.
    let pad = SAME_PLACE_MM.max((high - low).length() * 1e-3);
    let (low, high) = (low - Vec2::splat(pad), high + Vec2::splat(pad));
    vec![
        Vec2::new(low.x, low.y),
        Vec2::new(high.x, low.y),
        Vec2::new(high.x, high.y),
        Vec2::new(low.x, high.y),
    ]
}

/// One brace: the two points it runs between, its radius in millimetres, and the group
/// whose bracing it is built to.
pub(crate) type Brace = (Vec3, Vec3, Scalar, u16);

/// Every brace between the trunks of `trees`: end points, radius in mm and owning group.
/// A mixed pair takes the finer group's brace; a blocked brace is dropped (ADR 0252, 0077).
pub(crate) fn braces(trees: &[SupportTree], placed: &Placed, profiles: Profiles) -> Vec<Brace> {
    ties(trees, placed, profiles)
        .into_iter()
        .flat_map(|(_, _, struts)| struts)
        .collect()
}

/// Every pair of `trees` tied, by index, with the braces [`braces`] ties it with; a pair
/// the model stands in the way of everywhere is left out.
pub(crate) fn ties(
    trees: &[SupportTree],
    placed: &Placed,
    profiles: Profiles,
) -> Vec<(usize, usize, Vec<Brace>)> {
    // Each pair is tied on its own, so all are asked at once, kept in order.
    neighbours(trees, profiles)
        .into_par_iter()
        .filter_map(|(a, b)| {
            let (one, two) = (&trees[a], &trees[b]);
            let (group, profile) = lead_of(one, two, profiles);
            let struts: Vec<_> = tie(one, two, profiles)
                .into_iter()
                .filter(|&(from, to, radius_mm)| {
                    let beam = Beam {
                        from,
                        to,
                        from_radius_mm: radius_mm,
                        to_radius_mm: radius_mm,
                    }
                    .with_clearance(profile.clearance_mm);
                    clear(placed, &beam)
                })
                .map(|(from, to, radius_mm)| (from, to, radius_mm, group))
                .collect();
            (!struts.is_empty()).then_some((a, b, struts))
        })
        .collect()
}

/// Whether `tree` stands tall enough to carry one level of the bracing of `profile`,
/// which is its own group's.
pub(crate) fn braceable(tree: &SupportTree, profile: &SupportProfile) -> bool {
    // A tree leant onto another support is tied to it already.
    let bracing = &profile.bracing;
    bracing.enabled
        && bracing.rise_mm > SAME_PLACE_MM
        && !tree.landing().on_support
        && top_z(tree, profile) - tree.landing().bottom_z(profile)
            >= bracing.start_height_mm + bracing.rise_mm
}

/// Where a trunk ends at the top, which is as high as a brace or a buttress reaches: at
/// its root, or under the head of a column that holds its one tip on the trunk itself.
pub(crate) fn top_z(tree: &SupportTree, profile: &SupportProfile) -> Scalar {
    let root = tree.root();
    if root.is_leaf() {
        root.position.z - profile.top.length_mm
    } else {
        root.position.z
    }
}

/// The group a brace between `one` and `two` is built to, and its profile: the one whose
/// brace is the thinner, and the lower-numbered of two that are alike.
fn lead_of<'a>(
    one: &SupportTree,
    two: &SupportTree,
    profiles: Profiles<'a>,
) -> (u16, &'a SupportProfile) {
    let (a, b) = (one.group().min(two.group()), one.group().max(two.group()));
    let (first, second) = (profiles.of(a), profiles.of(b));
    if second.bracing.diameter_mm < first.bracing.diameter_mm {
        (b, second)
    } else {
        (a, first)
    }
}

/// The pairs of trunks worth tying: the Gabriel graph of the trunks tall enough to brace, cut at
/// the shorter span their groups allow; planar and keeps nearest neighbours (ADR 0240).
fn neighbours(trees: &[SupportTree], profiles: Profiles) -> Vec<(usize, usize)> {
    let reach = |index: usize| profiles.of(trees[index].group()).bracing.max_spacing_mm;
    let at = |index: usize| trees[index].root().position.truncate();
    let tall: Vec<usize> = (0..trees.len())
        .filter(|&index| braceable(&trees[index], profiles.of(trees[index].group())))
        .collect();
    let widest = tall
        .iter()
        .map(|&index| reach(index))
        .fold(0.0, Scalar::max);
    if widest <= 0.0 {
        return Vec::new();
    }
    let mut buckets = Buckets::new(widest);
    for &index in &tall {
        buckets.insert(at(index), index);
    }

    // Nothing stands inside the circle a pair spans: within half the pair's span of its middle.
    let gabriel = |one: usize, two: usize| {
        let (here, there) = (at(one), at(two));
        let (middle, half) = (
            (here + there) / 2.0,
            Vec2::splat((there - here).length() / 2.0),
        );
        (there - here).length() <= reach(one).min(reach(two))
            && !buckets.within(middle - half, middle + half).any(|&other| {
                let point = at(other);
                (point - here).dot(point - there) < -SAME_PLACE_MM
            })
    };
    let mut pairs: Vec<(usize, usize)> = tall
        .par_iter()
        .flat_map_iter(|&one| {
            let span = Vec2::splat(widest);
            buckets
                .within(at(one) - span, at(one) + span)
                .filter(move |&&two| two > one && gabriel(one, two))
                .map(move |&two| (one, two))
        })
        .collect();
    pairs.sort_unstable();
    pairs
}

/// The braces between one pair of trunks, built to the pair's leading profile and never wider
/// than either trunk where it meets it (ADR 0242).
fn tie(one: &SupportTree, two: &SupportTree, profiles: Profiles) -> Vec<(Vec3, Vec3, Scalar)> {
    let (_, lead) = lead_of(one, two, profiles);
    let bracing = &lead.bracing;
    let own = |tree: &SupportTree| profiles.of(tree.group());
    let width = |tree: &SupportTree, z: Scalar| {
        trunk_radius_at_mm(tree, own(tree), z).min(bracing.radius_mm())
    };
    let (here, there) = (one.root().position, two.root().position);
    if Vec2::new(there.x - here.x, there.y - here.y).length() < SAME_PLACE_MM {
        return Vec::new();
    }

    // A brace ties the two trunks, so it can only go where both of them are: above what
    // each stands on and below the joint each one's branches or head leave from.
    let floor = one
        .landing()
        .bottom_z(own(one))
        .max(two.landing().bottom_z(own(two)))
        + bracing.start_height_mm;
    let ceiling = top_z(one, own(one)).min(top_z(two, own(two)));

    // A brace runs from the foot of one trunk's level to the head of the other's (ADR
    // 0044), each end on its trunk's own axis at that height: a hand may have leant it.
    let on = |tree: &SupportTree, z: Scalar| trunk_axis_at(tree, own(tree), z);
    let mut struts = Vec::new();
    for level in brace_levels(bracing, floor, ceiling) {
        let (z, up) = (level.z_mm, level.z_mm + bracing.rise_mm);
        let (a, b) = if level.flipped {
            (two, one)
        } else {
            (one, two)
        };
        struts.push((on(a, z), on(b, up), width(a, z).min(width(b, up))));
        if level.pattern == BracePattern::Cross {
            struts.push((on(b, z), on(a, up), width(b, z).min(width(a, up))));
        }
    }
    struts
}

/// Which of the three zones up a pair of trunks a level of bracing belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BraceBand {
    Bottom,
    Middle,
    Top,
}

/// One level of bracing up a pair of trunks: one rise from `z_mm` up, tied as `pattern`,
/// and leaning the other way from its neighbour below when `flipped`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BraceLevel {
    pub z_mm: Scalar,
    pub band: BraceBand,
    pub pattern: BracePattern,
    pub flipped: bool,
}

/// The levels of `bracing` between `floor_mm` and `ceiling_mm`, lowest first; each zone takes
/// only the levels there is room for, the bottom first (ADR 0329).
pub fn brace_levels(bracing: &Bracing, floor_mm: Scalar, ceiling_mm: Scalar) -> Vec<BraceLevel> {
    let rise = bracing.rise_mm;
    if rise <= SAME_PLACE_MM {
        return Vec::new();
    }
    // Rounding must not lose a level that fits to the last micrometre.
    let fits = |room: Scalar| ((room + SAME_PLACE_MM) / rise).floor().max(0.0) as u32;
    let bottom = bracing.bottom.levels.min(fits(ceiling_mm - floor_mm));
    let above = ceiling_mm - floor_mm - bottom as Scalar * rise;
    let top = bracing.top.levels.min(fits(above));
    let middle = fits(above - top as Scalar * rise);

    let up = |count: u32, from: u32, band, pattern| {
        (0..count).map(move |index| BraceLevel {
            z_mm: floor_mm + (from + index) as Scalar * rise,
            band,
            pattern,
            flipped: (from + index) % 2 == 1,
        })
    };
    let down = (0..top).rev().map(|index| BraceLevel {
        z_mm: ceiling_mm - (index + 1) as Scalar * rise,
        band: BraceBand::Top,
        pattern: bracing.top.pattern,
        flipped: index % 2 == 1,
    });
    up(bottom, 0, BraceBand::Bottom, bracing.bottom.pattern)
        .chain(up(middle, bottom, BraceBand::Middle, bracing.middle))
        .chain(down)
        .collect()
}

#[cfg(test)]
mod tests;
