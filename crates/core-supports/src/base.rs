use crate::placed::Placed;
use core_geometry::{Mesh, Scalar, Vec2, Vec3, convex_hull};
use printer_profiles::{BracePattern, Bracing, SupportProfile};

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

/// Every brace between the trunks of `trees`: the pair of points it runs between, its
/// radius in millimetres, and the group whose bracing it is built to.
///
/// Each trunk is tied the way its own group's `[bracing]` says, and only where that is
/// switched on. A pair of one group is tied by that group's numbers. A pair of two
/// groups is tied by the finer of the two — the thinner brace, with its start and its
/// rise — no further apart than both let a brace span, and the brace is that group's:
/// the trunk less able to carry a thick brace is the one that says how thick it is. See
/// `docs/decisions/0252`.
///
/// A brace is a support body like any other, so one the model is in the way of is not
/// tied at all; see `docs/decisions/0077`.
pub(crate) fn braces(
    trees: &[SupportTree],
    placed: &Placed,
    profiles: Profiles,
) -> Vec<(Vec3, Vec3, Scalar, u16)> {
    let mut struts = Vec::new();
    for (one, two) in neighbours(trees, profiles) {
        let (group, profile) = lead_of(one, two, profiles);
        let tied = tie(one, two, profiles).into_iter();
        struts.extend(
            tied.filter(|&(from, to, radius_mm)| {
                let beam = Beam {
                    from,
                    to,
                    from_radius_mm: radius_mm,
                    to_radius_mm: radius_mm,
                }
                .with_clearance(profile.clearance_mm);
                clear(placed, &beam)
            })
            .map(|(from, to, radius_mm)| (from, to, radius_mm, group)),
        );
    }
    struts
}

/// Whether `tree` stands tall enough to carry one level of the bracing of `profile`,
/// which is its own group's.
pub(crate) fn braceable(tree: &SupportTree, profile: &SupportProfile) -> bool {
    // A tree leant onto another support is tied to it already.
    let bracing = &profile.bracing;
    bracing.enabled
        && bracing.rise_mm > SAME_PLACE_MM
        && !tree.landing().on_support
        && tree.root().position.z - tree.landing().bottom_z(profile)
            >= bracing.start_height_mm + bracing.rise_mm
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

/// The pairs of trunks worth tying: the Gabriel graph of the trunks tall enough to brace,
/// cut at the shorter of the two spans their groups allow.
///
/// Two trunks are neighbours when no third stands inside the circle they span as a
/// diameter. That graph is planar, so no two crosses cross each other seen from above,
/// and it keeps every trunk's nearest neighbour, so nothing a tie would reach is left
/// alone; tying every pair within reach puts three crosses where two hold as well. See
/// `docs/decisions/0240`.
fn neighbours<'a>(
    trees: &'a [SupportTree],
    profiles: Profiles,
) -> Vec<(&'a SupportTree, &'a SupportTree)> {
    let own = |tree: &SupportTree| profiles.of(tree.group());
    let tall: Vec<&SupportTree> = trees
        .iter()
        .filter(|tree| braceable(tree, own(tree)))
        .collect();
    let at = |tree: &SupportTree| tree.root().position.truncate();

    let mut pairs = Vec::new();
    for (index, one) in tall.iter().enumerate() {
        for two in &tall[index + 1..] {
            let (here, there) = (at(one), at(two));
            let reach = own(one)
                .bracing
                .max_spacing_mm
                .min(own(two).bracing.max_spacing_mm);
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

/// The braces between one pair of trunks, built to the pair's leading profile.
///
/// A brace is never wider than either trunk where it meets it, or it swallows the thinner
/// one; see `docs/decisions/0242`.
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
    // each stands on and below the joint each one's branches leave from.
    let floor = one
        .landing()
        .bottom_z(own(one))
        .max(two.landing().bottom_z(own(two)))
        + bracing.start_height_mm;
    let ceiling = here.z.min(there.z);

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

/// The levels of `bracing` between `floor_mm` and `ceiling_mm`, lowest first: the bottom
/// zone's up from the floor, the top zone's down from the ceiling, the middle's filling
/// what is left from the bottom zone up. A zone takes only the levels there is room for,
/// the bottom first; see `docs/decisions/0329`.
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
    ) -> Vec<(Vec3, Vec3, Scalar, u16)> {
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

        for (from, to, _, _) in &struts {
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
    fn a_brace_ends_on_a_trunk_a_hand_has_leant_and_not_where_it_was_grown() {
        let profile = with_bracing();
        let mut trees = standing(&[(10.0, 10.0), (18.0, 10.0)], 30.0, &profile);
        // The first trunk's foot carried 4 mm away from the other: it now leans.
        trees[0].move_landing(Vec3::new(6.0, 10.0, 0.0));

        let struts = tied(&trees, &Mesh::default(), &profile);
        assert!(!struts.is_empty(), "the pair is still within reach");
        for end in struts.iter().flat_map(|(from, to, _, _)| [*from, *to]) {
            let tree = if end.x < 14.0 { &trees[0] } else { &trees[1] };
            let on_it = trunk_axis_at(tree, &profile, end.z);
            assert!(
                (end - on_it).length() < 1e-4,
                "a brace ends on the trunk it ties, {end} against the trunk at {on_it}"
            );
        }
        assert!(
            struts
                .iter()
                .any(|(from, _, _, _)| (from.x - 10.0).abs() > 0.5 && from.x < 10.0),
            "low down, the leant trunk is not under its root any more"
        );
    }

    /// Two trunks 30 mm tall and 8 mm apart, the first in group `one` and the second in
    /// group `two` of `table`, with every brace between them.
    fn tied_across(table: &[SupportProfile], one: u16, two: u16) -> Vec<(Vec3, Vec3, Scalar, u16)> {
        let points = [
            SupportPoint::new(Vec3::new(10.0, 10.0, 30.0)).in_group(one),
            SupportPoint::new(Vec3::new(18.0, 10.0, 30.0)).in_group(two),
        ];
        let profiles = Profiles::new(table).expect("a table of groups");
        let model = Mesh::default();
        let trees = trees_on(&points, &model, Transform::default(), profiles);
        braces(
            &trees,
            &Placed::new(&model, &Bvh::build(&model), Transform::default()),
            profiles,
        )
    }

    /// Bracing as `with_bracing` has it, the braces `diameter_mm` wide.
    fn braced(diameter_mm: Scalar) -> SupportProfile {
        let mut profile = with_bracing();
        profile.bracing.diameter_mm = diameter_mm;
        profile
    }

    /// The same shape with the bracing switched off.
    fn unbraced() -> SupportProfile {
        let mut profile = with_bracing();
        profile.bracing.enabled = false;
        profile
    }

    #[test]
    fn a_group_is_braced_by_its_own_settings_whatever_the_first_group_says() {
        let struts = tied_across(&[unbraced(), braced(0.8)], 1, 1);
        assert!(
            !struts.is_empty(),
            "the second group has bracing on, and both trunks are its own"
        );
        assert!(
            struts.iter().all(|(_, _, _, group)| *group == 1),
            "and the braces are that group's, not the first one's"
        );
        assert!(
            tied_across(&[braced(0.8), unbraced()], 1, 1).is_empty(),
            "bracing on in the first group ties nothing in a group that has it off"
        );
    }

    #[test]
    fn two_groups_are_tied_by_the_finer_brace_and_only_where_both_are_braced() {
        let struts = tied_across(&[braced(0.8), braced(0.4)], 0, 1);
        assert!(!struts.is_empty(), "both groups brace, so the pair is tied");
        for (_, _, radius_mm, group) in &struts {
            assert_eq!(*group, 1, "the brace is the group's with the thinner one");
            assert!(
                *radius_mm <= 0.2 + 1e-6,
                "and no wider than that group's, got {radius_mm}"
            );
        }
        assert!(
            tied_across(&[braced(0.8), unbraced()], 0, 1).is_empty(),
            "a trunk whose group does not brace is not tied to one that does"
        );
    }

    #[test]
    fn a_pair_is_tied_no_further_apart_than_both_groups_span() {
        let mut near = braced(0.8);
        near.bracing.max_spacing_mm = 6.0;
        assert!(
            tied_across(&[braced(0.8), near], 0, 1).is_empty(),
            "8 mm is inside one group's span and outside the other's"
        );
    }

    #[test]
    fn a_brace_is_never_wider_than_the_trunks_it_ties() {
        let mut profile = with_bracing();
        profile.bracing.diameter_mm = profile.middle.diameter_mm * 3.0;
        let trees = standing(&[(10.0, 10.0), (18.0, 10.0)], 30.0, &profile);
        let struts = tied(&trees, &Mesh::default(), &profile);
        assert!(!struts.is_empty(), "two trunks within reach get tied");

        for &(from, to, radius_mm, _) in &struts {
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

    /// Bracing of `rise_mm` levels zoned as the standard preset is: one zigzag at the
    /// bottom, one cross at the top, zigzags between.
    fn zoned(rise_mm: Scalar) -> Bracing {
        Bracing {
            rise_mm,
            ..printer_profiles::BracingPreset::Standard.bracing()
        }
    }

    #[test]
    fn levels_without_zones_climb_crossed_from_the_floor() {
        let bracing = Bracing {
            rise_mm: 4.0,
            ..Bracing::default()
        };
        let levels = brace_levels(&bracing, 2.0, 15.0);
        let heights: Vec<Scalar> = levels.iter().map(|level| level.z_mm).collect();
        assert_eq!(
            heights,
            [2.0, 6.0, 10.0],
            "13 mm of room holds three 4 mm levels"
        );
        assert!(levels.iter().all(|l| l.pattern == BracePattern::Cross));
    }

    #[test]
    fn the_top_zone_hangs_from_the_ceiling_and_the_middle_fills_below_it() {
        let levels = brace_levels(&zoned(4.0), 0.0, 19.0);
        let heights: Vec<Scalar> = levels.iter().map(|level| level.z_mm).collect();
        // One bottom level at 0, then 19 − 4 − 4 = 11 mm holds two middle levels at 4 and
        // 8, and the top level runs from 15 to the ceiling at 19.
        assert_eq!(heights, [0.0, 4.0, 8.0, 15.0]);
        let patterns: Vec<BracePattern> = levels.iter().map(|level| level.pattern).collect();
        let (zigzag, cross) = (BracePattern::Zigzag, BracePattern::Cross);
        assert_eq!(patterns, [zigzag, zigzag, zigzag, cross]);
        let bands: Vec<BraceBand> = levels.iter().map(|level| level.band).collect();
        let (bottom, middle, top) = (BraceBand::Bottom, BraceBand::Middle, BraceBand::Top);
        assert_eq!(bands, [bottom, middle, middle, top]);
        let flipped: Vec<bool> = levels[..3].iter().map(|level| level.flipped).collect();
        assert_eq!(
            flipped,
            [false, true, false],
            "a zigzag leans the other way each level, on from one zone into the next"
        );
    }

    #[test]
    fn a_pair_too_short_for_every_zone_fills_the_bottom_first() {
        let levels = brace_levels(&zoned(4.0), 0.0, 6.0);
        assert_eq!(levels.len(), 1, "6 mm holds one 4 mm level");
        assert_eq!(
            levels[0].pattern,
            BracePattern::Zigzag,
            "and it is the bottom's"
        );
    }

    #[test]
    fn a_zigzag_level_is_one_strut_and_a_crossed_one_two() {
        let profile = SupportProfile {
            bracing: Bracing {
                start_height_mm: 5.0,
                ..zoned(5.0)
            },
            ..with_bracing()
        };
        let trees = standing(&[(10.0, 10.0), (18.0, 10.0)], 30.0, &profile);
        let mut struts = tied(&trees, &Mesh::default(), &profile);
        struts.sort_by(|a, b| a.0.z.total_cmp(&b.0.z));
        let per_level: Vec<usize> = struts
            .chunk_by(|a, b| (a.0.z - b.0.z).abs() < 1e-3)
            .map(<[_]>::len)
            .collect();
        assert!(per_level.len() >= 3, "{per_level:?}");
        assert_eq!(per_level.last(), Some(&2), "the top level is crossed");
        assert!(
            per_level[..per_level.len() - 1].iter().all(|&n| n == 1),
            "every level below it zigzags: {per_level:?}"
        );
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
        for (from, to, _, _) in &struts {
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
