use super::*;
use crate::SupportPoint;
use crate::fixtures::{profile, trees_on};
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
fn a_part_is_met_as_far_along_the_ray_as_it_stands() {
    let profile = profile();
    let trees = standing(&[(10.0, 10.0)], &profile);
    let ray = Ray::new(Vec3::new(-50.0, 10.0, 10.0), Vec3::X);

    let (_, along) = grab_along(&trees, &ray, Profiles::single(&profile), 0.0).expect("it is hit");
    assert!(
        (along - 60.0).abs() < 1e-3,
        "the pillar's axis is 60 mm from where the ray starts, got {along}"
    );
}

#[test]
fn a_ray_passing_just_beside_a_pillar_still_takes_hold_of_it() {
    let profile = profile();
    let trees = standing(&[(10.0, 10.0)], &profile);
    let beside = 10.0 + profile.pillar_radius_mm() + 0.3;
    let ray = Ray::new(Vec3::new(-50.0, beside, 10.0), Vec3::X);

    assert_eq!(
        grab(&trees, &ray, Profiles::single(&profile), 0.0),
        None,
        "0.3 mm outside the pillar is a miss when nothing is allowed for the aim"
    );
    assert_eq!(
        grab(&trees, &ray, Profiles::single(&profile), 0.5),
        Some(Grab {
            tree: 0,
            part: Part::Trunk
        }),
        "and a hit with half a millimetre allowed"
    );
}

#[test]
fn a_part_the_ray_passes_through_wins_over_a_nearer_one_it_only_passes_by() {
    let profile = profile();
    // The second pillar stands nearer the ray's origin and just off its line.
    let off = 10.0 + profile.pillar_radius_mm() + 0.3;
    let trees = standing(&[(30.0, 10.0), (10.0, off)], &profile);
    assert_eq!(trees.len(), 2, "20 mm apart is too far to share a trunk");
    let ray = Ray::new(Vec3::new(-50.0, 10.0, 10.0), Vec3::X);

    let held = grab(&trees, &ray, Profiles::single(&profile), 0.5).expect("one is hit");
    let through = trees
        .iter()
        .position(|tree| (tree.landing().base.x - 30.0).abs() < 1.0)
        .expect("the far pillar stands");
    assert_eq!(
        held.tree, through,
        "what is under the cursor is what was meant"
    );
}

#[test]
fn a_ray_through_the_pillar_finds_its_support() {
    let profile = profile();
    let trees = standing(&[(10.0, 10.0)], &profile);
    let ray = Ray::new(Vec3::new(-50.0, 10.0, 10.0), Vec3::X);
    assert_eq!(
        support_under(&trees, &ray, Profiles::single(&profile), 0.0),
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
        grab(&trees, &ray, Profiles::single(&profile), 0.0),
        Some(Grab {
            tree: 0,
            part: Part::Trunk
        })
    );
    assert_eq!(
        support_under(&trees, &ray, Profiles::single(&profile), 0.0),
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
    let held = grab(&trees, &ray, Profiles::single(&profile), 0.0).expect("the strut is there");

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
        grab(&trees, &ray, Profiles::single(&profile), 0.0),
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
        support_under(&trees, &ray, Profiles::single(&profile), 0.0),
        None
    );
}

#[test]
fn the_nearer_of_two_supports_in_line_wins() {
    let profile = profile();
    let trees = standing(&[(50.0, 10.0), (10.0, 10.0)], &profile);
    let ray = Ray::new(Vec3::new(-50.0, 10.0, 10.0), Vec3::X);
    assert_eq!(
        support_under(&trees, &ray, Profiles::single(&profile), 0.0),
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
        support_under(&trees, &ray, Profiles::single(&profile), 0.0),
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
        support_under(&trees, &ray, Profiles::single(&profile), 0.0),
        None
    );
}

#[test]
fn a_ray_down_the_supports_own_axis_still_hits_it() {
    let profile = profile();
    let trees = standing(&[(10.0, 10.0)], &profile);
    let ray = Ray::new(Vec3::new(10.0, 10.0, 60.0), -Vec3::Z);
    assert_eq!(
        support_under(&trees, &ray, Profiles::single(&profile), 0.0),
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
        support_under(&trees, &ray, Profiles::single(&profile), 0.0),
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
        support_under(&trees, &ray, Profiles::single(&profile), 0.0),
        None
    );
}

#[test]
fn nothing_is_under_a_ray_when_there_are_no_supports() {
    let ray = Ray::new(Vec3::ZERO, Vec3::X);
    assert_eq!(
        support_under(&[], &ray, Profiles::single(&profile()), 0.0),
        None
    );
}
