use crate::placed::Placed;
use core_geometry::{Mesh, Scalar, Vec2, Vec3};
use printer_profiles::SupportProfile;

use crate::clear::{Beam, clear};
use crate::group::Profiles;
use crate::mesher::trunk_radius_at_mm;
use crate::tree::SupportTree;

/// Two feet closer together than this stand in the same place, and the hull cannot tell
/// them apart. Well under one pixel of any panel.
pub(crate) const SAME_PLACE_MM: Scalar = 1e-4;

/// How long a skate is, and how wide, as multiples of the foot's own radius. A skate
/// grips along its length and releases across its width, so the two are not the same.
pub const SKATE_LENGTH: Scalar = 1.6;
pub const SKATE_WIDTH: Scalar = 0.55;

/// How far in the bevel pulls the skate's footprint, as a fraction of its own size. The
/// pad lies flat on the plate and meets it on a rim a blade gets under; see
/// `docs/decisions/0044`.
pub const SKATE_BEVEL: Scalar = 0.3;

/// Half the width of a skate across its narrow way, millimetres, which is the most a
/// flare standing in one may spread to.
pub(crate) fn skate_half_width_mm(profile: &SupportProfile) -> Scalar {
    profile.base_radius_mm() * SKATE_WIDTH * (1.0 - SKATE_BEVEL)
}

/// One skate foot: a pad lying flat on the plate, bevelled all the way round.
///
/// `toe` is the horizontal direction it is elongated along, which is away from the part
/// the support came down from; see `docs/decisions/0043`.
pub(crate) fn skate(mesh: &mut Mesh, base: Vec3, toe: Vec3, profile: &SupportProfile) {
    let thickness = profile.base_height_mm();
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

/// Which way the toe of `tree`'s skate points: away from the part it came down from.
///
/// A support that never leaned has no such direction, and the plate's own x is as good a
/// way to point as any.
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

/// The convex hull of `points`, counter-clockwise, by Andrew's monotone chain.
pub(crate) fn hull(points: &[Vec2]) -> Vec<Vec2> {
    if points.len() < 3 {
        return rectangle(points);
    }

    let mut sorted = points.to_vec();
    sorted.sort_by(|a, b| a.x.total_cmp(&b.x).then_with(|| a.y.total_cmp(&b.y)));
    sorted.dedup_by(|a, b| (*a - *b).length() < SAME_PLACE_MM);
    if sorted.len() < 3 {
        return rectangle(points);
    }

    let mut chain: Vec<Vec2> = Vec::with_capacity(sorted.len() * 2);
    for pass in 0..2 {
        let start = chain.len();
        let walk: Box<dyn Iterator<Item = &Vec2>> = if pass == 0 {
            Box::new(sorted.iter())
        } else {
            Box::new(sorted.iter().rev())
        };
        for &point in walk {
            while chain.len() >= start + 2
                && !turns_left(chain[chain.len() - 2], chain[chain.len() - 1], point)
            {
                chain.pop();
            }
            chain.push(point);
        }
        chain.pop();
    }

    if chain.len() < 3 {
        rectangle(points)
    } else {
        chain
    }
}

/// Whether `c` lies to the left of the line `a`..`b`, which is what keeps a hull convex.
fn turns_left(a: Vec2, b: Vec2, c: Vec2) -> bool {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x) > SAME_PLACE_MM
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

/// Every brace between the trunks of `trees`, as the pair of points it runs between and
/// its radius in millimetres.
///
/// Trunks within `max_spacing_mm` of each other are tied at `rise_mm` intervals from
/// `start_height_mm` up, as high as both of them still stand. See `docs/decisions/0043`.
/// A brace is a support body like any other, so one the model is in the way of is not
/// tied at all; see `docs/decisions/0077`. The bracing is the ground profile's, and each
/// trunk is as wide as its own group's profile makes it.
pub(crate) fn braces(
    trees: &[SupportTree],
    placed: &Placed,
    profiles: Profiles,
) -> Vec<(Vec3, Vec3, Scalar)> {
    let profile = profiles.ground();
    let bracing = &profile.bracing;
    if !bracing.enabled || bracing.rise_mm <= SAME_PLACE_MM {
        return Vec::new();
    }

    let mut struts = Vec::new();
    for (one, two) in neighbours(trees, profile) {
        let tied = tie(one, two, profiles).into_iter();
        struts.extend(tied.filter(|&(from, to, radius_mm)| {
            let beam = Beam {
                from,
                to,
                from_radius_mm: radius_mm,
                to_radius_mm: radius_mm,
            }
            .with_clearance(profile.clearance_mm);
            clear(placed, &beam)
        }));
    }
    struts
}

/// Whether `tree` stands tall enough to carry one level of bracing.
pub(crate) fn braceable(tree: &SupportTree, profile: &SupportProfile) -> bool {
    // A tree leant onto another support is tied to it already.
    let bracing = &profile.bracing;
    !tree.landing().on_support
        && tree.root().position.z - tree.landing().bottom_z(profile)
            >= bracing.start_height_mm + bracing.rise_mm
}

/// The pairs of trunks worth tying: the Gabriel graph of the trunks tall enough to brace,
/// cut at `max_spacing_mm`.
///
/// Two trunks are neighbours when no third stands inside the circle they span as a
/// diameter. That graph is planar, so no two crosses cross each other seen from above,
/// and it keeps every trunk's nearest neighbour, so nothing a tie would reach is left
/// alone; tying every pair within reach puts three crosses where two hold as well. See
/// `docs/decisions/0240`.
fn neighbours<'a>(
    trees: &'a [SupportTree],
    profile: &SupportProfile,
) -> Vec<(&'a SupportTree, &'a SupportTree)> {
    let tall: Vec<&SupportTree> = trees
        .iter()
        .filter(|tree| braceable(tree, profile))
        .collect();
    let at = |tree: &SupportTree| tree.root().position.truncate();
    let reach = profile.bracing.max_spacing_mm;

    let mut pairs = Vec::new();
    for (index, one) in tall.iter().enumerate() {
        for two in &tall[index + 1..] {
            let (here, there) = (at(one), at(two));
            if (there - here).length() > reach {
                continue;
            }
            let between = tall.iter().any(|other| {
                let point = at(other);
                (point - here).dot(point - there) < -SAME_PLACE_MM
            });
            if !between {
                pairs.push((*one, *two));
            }
        }
    }
    pairs
}

/// The braces between one pair of trunks.
///
/// A brace is never wider than either trunk where it meets it, or it swallows the thinner
/// one; see `docs/decisions/0242`.
fn tie(one: &SupportTree, two: &SupportTree, profiles: Profiles) -> Vec<(Vec3, Vec3, Scalar)> {
    let profile = profiles.ground();
    let bracing = &profile.bracing;
    let width = |tree: &SupportTree, z: Scalar| {
        trunk_radius_at_mm(tree, profiles.of(tree.group()), z).min(bracing.radius_mm())
    };
    let (here, there) = (one.root().position, two.root().position);
    let ground = Vec2::new(there.x - here.x, there.y - here.y).length();
    if ground > bracing.max_spacing_mm || ground < SAME_PLACE_MM {
        return Vec::new();
    }

    // A brace ties the two trunks, so it can only go where both of them are: above what
    // each stands on and below the joint each one's branches leave from.
    let floor = one
        .landing()
        .bottom_z(profile)
        .max(two.landing().bottom_z(profile))
        + bracing.start_height_mm;
    let ceiling = here.z.min(there.z);

    // A cross rather than a rung: each step ties the foot of one trunk to the head of
    // the other and back again, so the pair is braced against leaning either way. See
    // `docs/decisions/0044`.
    let mut struts = Vec::new();
    let mut z = floor;
    while z + bracing.rise_mm <= ceiling {
        let up = z + bracing.rise_mm;
        struts.push((
            Vec3::new(here.x, here.y, z),
            Vec3::new(there.x, there.y, up),
            width(one, z).min(width(two, up)),
        ));
        struts.push((
            Vec3::new(there.x, there.y, z),
            Vec3::new(here.x, here.y, up),
            width(two, z).min(width(one, up)),
        ));
        z = up;
    }
    struts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Profiles;
    use crate::SupportPoint;
    use crate::tests::{box_mesh, profile, trees_on};
    use core_geometry::{Bvh, Mesh, Transform};
    use core_geometry::{diagnose, signed_volume};
    use printer_profiles::{Bracing, PlatformShape};

    /// Every brace between `trees`, with `model` in the way of them.
    fn tied(
        trees: &[SupportTree],
        model: &Mesh,
        profile: &SupportProfile,
    ) -> Vec<(Vec3, Vec3, Scalar)> {
        braces(
            trees,
            &Placed::new(model, &Bvh::build(model), Transform::default()),
            Profiles::single(profile),
        )
    }

    /// Supports standing on the plate at each of `places`, `height` millimetres tall and
    /// far enough apart not to merge.
    fn standing(
        places: &[(Scalar, Scalar)],
        height: Scalar,
        profile: &SupportProfile,
    ) -> Vec<SupportTree> {
        let points: Vec<SupportPoint> = places
            .iter()
            .map(|(x, y)| SupportPoint::new(Vec3::new(*x, *y, height)))
            .collect();
        trees_on(
            &points,
            &Mesh::default(),
            Transform::default(),
            Profiles::single(profile),
        )
    }

    /// Bracing on and branching off, so that the two trunks the test needs stay two.
    fn with_bracing() -> SupportProfile {
        SupportProfile {
            branching: printer_profiles::Branching {
                enabled: false,
                ..profile().branching
            },
            bracing: Bracing {
                enabled: true,
                start_height_mm: 5.0,
                rise_mm: 5.0,
                max_spacing_mm: 12.0,
                ..Bracing::default()
            },
            ..profile()
        }
    }

    #[test]
    fn a_skate_lies_flat_on_the_plate_and_is_bevelled_round_its_rim() {
        let profile = SupportProfile {
            bottom: printer_profiles::BottomSegment {
                shape: PlatformShape::Skate,
                ..profile().bottom
            },
            ..profile()
        };
        let mut mesh = Mesh::default();
        skate(&mut mesh, Vec3::new(10.0, 10.0, 0.0), Vec3::X, &profile);

        let diagnostics = diagnose(&mesh);
        assert_eq!(diagnostics.boundary_edges, 0, "a skate is a closed solid");
        assert_eq!(diagnostics.non_manifold_edges, 0);
        assert_eq!(diagnostics.degenerate_faces, 0);
        assert!(signed_volume(&mesh) > 0.0, "it is wound outwards");

        let reach = |z: Scalar| {
            mesh.vertices
                .iter()
                .filter(|vertex| (vertex.z - z).abs() < 1e-4)
                .map(|vertex| vertex.x - 10.0)
                .fold(Scalar::NEG_INFINITY, Scalar::max)
        };
        assert!(
            mesh.vertices.iter().filter(|v| v.z < 1e-4).count() == 4,
            "the pad lies flat: four corners on the plate and no others"
        );
        assert!(
            reach(0.0) < reach(profile.base_height_mm()) - 1e-3,
            "the pad meets the plate on a rim pulled in from its widest point: {} \
             against {}",
            reach(0.0),
            reach(profile.base_height_mm())
        );
    }

    #[test]
    fn a_skate_points_away_from_the_part_it_came_down_from() {
        let profile = profile();
        // Two tips that merge, so the trunk sits between them and each branch leans.
        let trees = standing(&[(10.0, 10.0), (14.0, 10.0)], 20.0, &profile);
        let toe = toe_of(&trees[0]);
        assert!(
            (toe.length() - 1.0).abs() < 1e-4 && toe.z.abs() < 1e-6,
            "the toe points along the plate, got {toe}"
        );
    }

    #[test]
    fn a_column_that_never_leaned_still_has_a_toe_to_point() {
        let profile = profile();
        let trees = standing(&[(10.0, 10.0)], 20.0, &profile);
        assert_eq!(toe_of(&trees[0]), Vec3::X);
    }

    #[test]
    fn neighbouring_trunks_are_tied_at_every_rise() {
        let profile = with_bracing();
        let trees = standing(&[(10.0, 10.0), (18.0, 10.0)], 30.0, &profile);
        assert_eq!(trees.len(), 2, "branching is off, so they stay two trunks");

        let struts = tied(&trees, &Mesh::default(), &profile);
        assert!(!struts.is_empty(), "two trunks within reach get tied");
        assert_eq!(struts.len() % 2, 0, "a cross is two struts, never one");

        for (from, to, _) in &struts {
            assert!(
                (to.z - from.z - profile.bracing.rise_mm).abs() < 1e-4,
                "a brace climbs one rise on its way across, got {from} to {to}"
            );
            assert!(
                from.z >= profile.bracing.start_height_mm - 1e-4,
                "no brace below the start height, got {}",
                from.z
            );
        }

        // The two struts of one cross run opposite ways between the same two trunks:
        // same pair of heights, swapped pair of trunks.
        let (first, second) = (struts[0], struts[1]);
        let ground = |p: Vec3| Vec2::new(p.x, p.y);
        assert!((ground(first.0) - ground(second.1)).length() < 1e-4);
        assert!((ground(first.1) - ground(second.0)).length() < 1e-4);
        assert!((first.0.z - second.0.z).abs() < 1e-4);
    }

    #[test]
    fn a_brace_is_never_wider_than_the_trunks_it_ties() {
        let mut profile = with_bracing();
        profile.bracing.diameter_mm = profile.middle.diameter_mm * 3.0;
        let trees = standing(&[(10.0, 10.0), (18.0, 10.0)], 30.0, &profile);
        let struts = tied(&trees, &Mesh::default(), &profile);
        assert!(!struts.is_empty(), "two trunks within reach get tied");

        for &(from, to, radius_mm) in &struts {
            let at = |point: Vec3| {
                let tree = trees
                    .iter()
                    .find(|tree| {
                        (tree.root().position.truncate() - point.truncate()).length() < 1e-3
                    })
                    .expect("a brace ends on one of the two trunks");
                trunk_radius_at_mm(tree, &profile, point.z)
            };
            assert!(
                radius_mm <= at(from) + 1e-6 && radius_mm <= at(to) + 1e-6,
                "a {radius_mm} mm brace is no wider than the trunks it meets, {} and {} mm",
                at(from),
                at(to)
            );
            assert!(
                radius_mm < profile.bracing.radius_mm(),
                "a brace thicker than the trunks is thinned, not drawn at {} mm",
                profile.bracing.radius_mm()
            );
        }
    }

    #[test]
    fn a_pair_with_room_for_only_one_level_is_not_tied() {
        let profile = with_bracing();
        // Tall enough for the first level but not for the rise above it, so no cross.
        let height = profile.bracing.start_height_mm + profile.bracing.rise_mm / 2.0;
        let trees = standing(&[(10.0, 10.0), (18.0, 10.0)], height, &profile);
        assert!(tied(&trees, &Mesh::default(), &profile).is_empty());
    }

    #[test]
    fn three_trunks_in_a_row_are_tied_to_their_neighbours_and_not_end_to_end() {
        // 4 mm apart with 12 mm of reach: every pair is within it, and the outer two have
        // the middle one standing between them.
        let profile = with_bracing();
        let row = standing(&[(10.0, 10.0), (14.0, 10.0), (18.0, 10.0)], 30.0, &profile);
        let struts = tied(&row, &Mesh::default(), &profile);

        assert!(!struts.is_empty(), "neighbours are tied");
        for (from, to, _) in &struts {
            assert!(
                (to.x - from.x).abs() < 4.0 + 1e-3,
                "a brace from {from} to {to} runs past the trunk between them"
            );
        }
        let pair = tied(&row[..2], &Mesh::default(), &profile);
        assert_eq!(
            struts.len(),
            2 * pair.len(),
            "two pairs of neighbours, each tied as a pair alone is"
        );
    }

    #[test]
    fn trunks_further_apart_than_the_spacing_are_not_tied() {
        let profile = with_bracing();
        let far = profile.bracing.max_spacing_mm + 5.0;
        let trees = standing(&[(10.0, 10.0), (10.0 + far, 10.0)], 30.0, &profile);
        assert!(tied(&trees, &Mesh::default(), &profile).is_empty());
    }

    #[test]
    fn bracing_that_is_switched_off_ties_nothing() {
        let profile = profile();
        let trees = standing(&[(10.0, 10.0), (18.0, 10.0)], 30.0, &profile);
        assert!(tied(&trees, &Mesh::default(), &profile).is_empty());
    }

    #[test]
    fn a_brace_the_model_stands_in_the_way_of_is_not_tied() {
        let profile = with_bracing();
        let trees = standing(&[(10.0, 10.0), (18.0, 10.0)], 30.0, &profile);
        // A wall between the two trunks, tall enough to cross every brace.
        let wall = box_mesh(Vec3::new(13.5, 5.0, 0.0), Vec3::new(14.5, 15.0, 30.0));

        assert!(!tied(&trees, &Mesh::default(), &profile).is_empty());
        assert!(
            tied(&trees, &wall, &profile).is_empty(),
            "a brace is a body like any other; see docs/decisions/0077"
        );
    }

    #[test]
    fn a_trunk_too_short_for_the_start_height_is_not_tied() {
        let profile = with_bracing();
        // Both trunks end well under the height the first brace would sit at.
        let trees = standing(&[(10.0, 10.0), (18.0, 10.0)], 3.0, &profile);
        assert!(tied(&trees, &Mesh::default(), &profile).is_empty());
    }
}
