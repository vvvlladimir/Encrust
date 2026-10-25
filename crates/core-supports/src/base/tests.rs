use super::*;
use crate::Profiles;
use crate::SupportPoint;
use crate::fixtures::{box_mesh, profile, trees_on};
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
        reach(0.0) < reach(profile.bottom.platform_thickness_mm) - 1e-3,
        "the pad meets the plate on a rim pulled in from its widest point: {} \
             against {}",
        reach(0.0),
        reach(profile.bottom.platform_thickness_mm)
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
                .find(|tree| (tree.root().position.truncate() - point.truncate()).length() < 1e-3)
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
fn a_brace_stops_under_the_head_of_a_column() {
    let profile = with_bracing();
    let trees = standing(&[(10.0, 10.0), (18.0, 10.0)], 30.0, &profile);
    let neck = 30.0 - profile.top.length_mm;
    let struts = tied(&trees, &Mesh::default(), &profile);
    assert!(!struts.is_empty());
    for (from, to, _, _) in &struts {
        assert!(
            from.z.max(to.z) <= neck + 1e-4,
            "a brace reaches no higher than where the head meets the trunk, {neck} mm"
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

#[test]
fn trunks_are_paired_as_the_gabriel_graph_of_every_pair_would_pair_them() {
    let profile = with_bracing();
    // A scatter on a 7 mm lattice, jittered so no three lie on one line.
    let places: Vec<(Scalar, Scalar)> = (0..36)
        .map(|index| {
            let (column, row) = ((index % 6) as Scalar, (index / 6) as Scalar);
            let jitter = ((index * 7919) % 13) as Scalar / 10.0;
            (
                10.0 + 7.0 * column + jitter,
                10.0 + 7.0 * row - jitter / 2.0,
            )
        })
        .collect();
    let trees = standing(&places, 30.0, &profile);
    assert_eq!(trees.len(), places.len(), "branching is off");

    let at = |index: usize| trees[index].root().position.truncate();
    let mut every = Vec::new();
    for one in 0..trees.len() {
        for two in one + 1..trees.len() {
            let (here, there) = (at(one), at(two));
            let empty = !(0..trees.len()).any(|other| {
                let point = at(other);
                (point - here).dot(point - there) < -SAME_PLACE_MM
            });
            if (there - here).length() <= profile.bracing.max_spacing_mm && empty {
                every.push((one, two));
            }
        }
    }
    assert!(!every.is_empty());
    assert_eq!(neighbours(&trees, Profiles::single(&profile)), every);
}
