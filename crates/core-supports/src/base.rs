use crate::placed::Placed;
use core_geometry::{Mesh, Scalar, Vec2, Vec3};
use printer_profiles::{RaftShape, SupportProfile};

use crate::clear::{Beam, clear};
use crate::tree::SupportTree;

/// Two feet closer together than this stand in the same place, and the hull cannot tell
/// them apart. Well under one pixel of any panel.
const SAME_PLACE_MM: Scalar = 1e-4;

/// How long a skate is, and how wide, as multiples of the foot's own radius. A skate
/// grips along its length and releases across its width, so the two are not the same.
const SKATE_LENGTH: Scalar = 1.6;
const SKATE_WIDTH: Scalar = 0.55;

/// How far in the bevel pulls the skate's footprint, as a fraction of its own size. The
/// pad lies flat on the plate and meets it on a rim a blade gets under; see
/// `docs/decisions/0044`.
const SKATE_BEVEL: Scalar = 0.3;

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

/// The raft under every support that reaches the plate, or `None` when there is nothing
/// to stand on it.
///
/// The slab runs from the plate up to `thickness_mm` and the supports pass straight
/// through it, which is the same overlap rule every joint uses; see
/// `docs/decisions/0041`.
pub(crate) fn raft(trees: &[SupportTree], profile: &SupportProfile) -> Option<Mesh> {
    let raft = &profile.raft;
    if !raft.enabled {
        return None;
    }

    let feet: Vec<Vec2> = trees
        .iter()
        .filter(|tree| !tree.landing().on_model)
        .map(|tree| {
            let base = tree.landing().base;
            Vec2::new(base.x, base.y)
        })
        .collect();
    if feet.is_empty() {
        return None;
    }

    let outline = match raft.shape {
        RaftShape::Hull => hull(&feet),
        RaftShape::Rectangle => rectangle(&feet),
    };
    // A foot's own width is part of what the raft has to cover, and so is the spread.
    let margin = profile.base_radius_mm();
    let top = grown(&outline, margin, raft.spread());
    (top.len() >= 3).then(|| slab(&top, raft.overhang_mm(), raft.thickness_mm))
}

/// The outline pushed `margin_mm` outwards and then spread about its own middle.
fn grown(outline: &[Vec2], margin_mm: Scalar, spread: Scalar) -> Vec<Vec2> {
    let middle = outline.iter().fold(Vec2::ZERO, |sum, &p| sum + p) / outline.len() as Scalar;
    outline
        .iter()
        .map(|&point| {
            let arm = point - middle;
            let out = if arm.length() < SAME_PLACE_MM {
                Vec2::ZERO
            } else {
                arm.normalize() * margin_mm
            };
            middle + arm * spread + out
        })
        .collect()
}

/// A closed slab: `outline` at `thickness_mm`, the same outline pulled `overhang_mm`
/// in where it meets the plate, and the walls between them.
///
/// The rim on the plate is the narrow one, which is what a blade gets under; see
/// `docs/decisions/0044`.
fn slab(outline: &[Vec2], overhang_mm: Scalar, thickness_mm: Scalar) -> Mesh {
    let mut mesh = Mesh::default();
    let count = outline.len();
    let middle = outline.iter().fold(Vec2::ZERO, |sum, &p| sum + p) / count as Scalar;

    let foot = grown(outline, -overhang_mm, 1.0);
    for point in &foot {
        mesh.vertices.push(Vec3::new(point.x, point.y, 0.0));
    }
    for point in outline {
        mesh.vertices
            .push(Vec3::new(point.x, point.y, thickness_mm));
    }
    let low_middle = mesh.vertices.len() as u32;
    mesh.vertices.push(Vec3::new(middle.x, middle.y, 0.0));
    let high_middle = mesh.vertices.len() as u32;
    mesh.vertices
        .push(Vec3::new(middle.x, middle.y, thickness_mm));

    let low = |index: usize| (index % count) as u32;
    let high = |index: usize| (count + index % count) as u32;
    for index in 0..count {
        mesh.faces.push([low_middle, low(index + 1), low(index)]);
        mesh.faces.push([high_middle, high(index), high(index + 1)]);
        mesh.faces
            .push([low(index), low(index + 1), high(index + 1)]);
        mesh.faces.push([low(index), high(index + 1), high(index)]);
    }
    mesh
}

/// The convex hull of `points`, counter-clockwise, by Andrew's monotone chain.
fn hull(points: &[Vec2]) -> Vec<Vec2> {
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

/// Every brace between the trunks of `trees`, as the pair of points it runs between.
///
/// Trunks within `max_spacing_mm` of each other are tied at `rise_mm` intervals from
/// `start_height_mm` up, as high as both of them still stand. See `docs/decisions/0043`.
/// A brace is a support body like any other, so one the model is in the way of is not
/// tied at all; see `docs/decisions/0077`.
pub(crate) fn braces(
    trees: &[SupportTree],
    placed: &Placed,
    profile: &SupportProfile,
) -> Vec<(Vec3, Vec3)> {
    let bracing = &profile.bracing;
    if !bracing.enabled || bracing.rise_mm <= SAME_PLACE_MM {
        return Vec::new();
    }

    let radius_mm = bracing.radius_mm();
    let mut struts = Vec::new();
    for (index, one) in trees.iter().enumerate() {
        for two in &trees[index + 1..] {
            struts.extend(tie(one, two, profile).into_iter().filter(|(from, to)| {
                let beam = Beam {
                    from: *from,
                    to: *to,
                    from_radius_mm: radius_mm,
                    to_radius_mm: radius_mm,
                }
                .with_clearance(profile.clearance_mm);
                clear(placed, &beam)
            }));
        }
    }
    struts
}

/// The braces between one pair of trunks.
fn tie(one: &SupportTree, two: &SupportTree, profile: &SupportProfile) -> Vec<(Vec3, Vec3)> {
    let bracing = &profile.bracing;
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
        ));
        struts.push((
            Vec3::new(there.x, there.y, z),
            Vec3::new(here.x, here.y, up),
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
    use printer_profiles::{Bracing, PlatformShape, Raft, RaftShape};

    /// Every brace between `trees`, with `model` in the way of them.
    fn tied(trees: &[SupportTree], model: &Mesh, profile: &SupportProfile) -> Vec<(Vec3, Vec3)> {
        braces(
            trees,
            &Placed::new(model, &Bvh::build(model), Transform::default()),
            profile,
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

    fn with_raft(shape: RaftShape) -> SupportProfile {
        SupportProfile {
            raft: Raft {
                enabled: true,
                shape,
                ..Raft::default()
            },
            ..profile()
        }
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
    fn a_raft_covers_every_foot_that_reaches_the_plate() {
        let profile = with_raft(RaftShape::Hull);
        let places = [(10.0, 10.0), (30.0, 10.0), (30.0, 30.0), (10.0, 30.0)];
        let trees = standing(&places, 20.0, &profile);
        assert_eq!(
            trees.len(),
            4,
            "twenty millimetres apart is too far to merge"
        );

        let slab = raft(&trees, &profile).expect("four feet make a raft");
        let bounds = slab.aabb().expect("the slab has vertices");
        for (x, y) in places {
            assert!(
                bounds.mins.x <= x && x <= bounds.maxs.x,
                "the raft has to reach the foot at {x}, {y}"
            );
            assert!(bounds.mins.y <= y && y <= bounds.maxs.y);
        }
        assert!(
            (bounds.maxs.z - profile.raft.thickness_mm).abs() < 1e-4,
            "the slab is its own thickness tall"
        );
        assert!(bounds.mins.z.abs() < 1e-4, "it sits on the plate");
    }

    #[test]
    fn a_raft_is_a_closed_solid_wound_outwards() {
        for shape in [RaftShape::Hull, RaftShape::Rectangle] {
            let profile = with_raft(shape);
            let trees = standing(&[(10.0, 10.0), (30.0, 10.0), (20.0, 30.0)], 20.0, &profile);
            let slab = raft(&trees, &profile).expect("three feet make a raft");

            let diagnostics = diagnose(&slab);
            assert_eq!(
                diagnostics.boundary_edges, 0,
                "{shape:?} left the raft open"
            );
            assert_eq!(diagnostics.non_manifold_edges, 0, "{shape:?}");
            assert_eq!(diagnostics.degenerate_faces, 0, "{shape:?}");
            assert!(signed_volume(&slab) > 0.0, "{shape:?}");
        }
    }

    #[test]
    fn a_raft_meets_the_plate_on_a_rim_pulled_in_from_its_widest_point() {
        let profile = with_raft(RaftShape::Rectangle);
        let trees = standing(&[(10.0, 10.0), (30.0, 10.0), (20.0, 30.0)], 20.0, &profile);
        let slab = raft(&trees, &profile).expect("a raft");

        let width_at = |z: Scalar| {
            let xs: Vec<Scalar> = slab
                .vertices
                .iter()
                .filter(|vertex| (vertex.z - z).abs() < 1e-3)
                .map(|vertex| vertex.x)
                .collect();
            xs.iter().fold(Scalar::NEG_INFINITY, |a, &b| a.max(b))
                - xs.iter().fold(Scalar::INFINITY, |a, &b| a.min(b))
        };
        assert!(
            width_at(0.0) < width_at(profile.raft.thickness_mm) - 1e-3,
            "a sloped wall leans in on the way down, so a blade gets under the rim"
        );
    }

    #[test]
    fn a_bigger_area_ratio_makes_a_bigger_raft() {
        let small = with_raft(RaftShape::Rectangle);
        let large = SupportProfile {
            raft: Raft {
                area_ratio: 3.0,
                ..small.raft
            },
            ..small.clone()
        };
        let places = [(10.0, 10.0), (30.0, 10.0), (20.0, 30.0)];

        let one = raft(&standing(&places, 20.0, &small), &small).expect("a raft");
        let other = raft(&standing(&places, 20.0, &large), &large).expect("a raft");
        assert!(signed_volume(&other) > signed_volume(&one));
    }

    #[test]
    fn a_support_that_stands_on_the_model_needs_no_raft() {
        let profile = with_raft(RaftShape::Hull);
        let shelf = box_mesh(Vec3::ZERO, Vec3::splat(10.0));
        let trees = trees_on(
            &[SupportPoint::new(Vec3::new(5.0, 5.0, 20.0))],
            &shelf,
            Transform::default(),
            Profiles::single(&profile),
        );
        assert!(trees[0].landing().on_model);
        assert!(
            raft(&trees, &profile).is_none(),
            "nothing reaches the plate"
        );
    }

    #[test]
    fn a_raft_that_is_switched_off_is_not_built() {
        let profile = profile();
        let trees = standing(&[(10.0, 10.0), (30.0, 10.0), (20.0, 30.0)], 20.0, &profile);
        assert!(raft(&trees, &profile).is_none());
    }

    #[test]
    fn neighbouring_trunks_are_tied_at_every_rise() {
        let profile = with_bracing();
        let trees = standing(&[(10.0, 10.0), (18.0, 10.0)], 30.0, &profile);
        assert_eq!(trees.len(), 2, "branching is off, so they stay two trunks");

        let struts = tied(&trees, &Mesh::default(), &profile);
        assert!(!struts.is_empty(), "two trunks within reach get tied");
        assert_eq!(struts.len() % 2, 0, "a cross is two struts, never one");

        for (from, to) in &struts {
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
    fn a_pair_with_room_for_only_one_level_is_not_tied() {
        let profile = with_bracing();
        // Tall enough for the first level but not for the rise above it, so no cross.
        let height = profile.bracing.start_height_mm + profile.bracing.rise_mm / 2.0;
        let trees = standing(&[(10.0, 10.0), (18.0, 10.0)], height, &profile);
        assert!(tied(&trees, &Mesh::default(), &profile).is_empty());
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
