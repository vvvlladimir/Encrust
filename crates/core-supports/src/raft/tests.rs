use super::*;
use crate::Profiles;
use crate::SupportPoint;
use crate::fixtures::{box_mesh, profile, trees_on};
use core_geometry::{Bvh, Transform, Vec3, diagnose, signed_volume, winding_number};
use printer_profiles::Raft;

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

fn with_raft(style: RaftStyle, shape: RaftShape) -> SupportProfile {
    SupportProfile {
        raft: Raft {
            enabled: true,
            style,
            shape,
            ..Raft::default()
        },
        ..profile()
    }
}

/// The raft under `trees` with `model` placed where it stands.
fn built(trees: &[SupportTree], model: &Mesh, profile: &SupportProfile) -> Option<Mesh> {
    raft(
        trees,
        &Placed::new(model, &Bvh::build(model), Transform::default()),
        Profiles::single(profile),
    )
}

/// Whether `point` lies inside any of the overlapping solids of `mesh`, whose winding
/// numbers add up.
fn inside(mesh: &Mesh, point: Vec3) -> bool {
    winding_number(mesh, point) > 0.5
}

const TRIANGLE: [(Scalar, Scalar); 3] = [(10.0, 10.0), (30.0, 10.0), (20.0, 30.0)];

#[test]
fn every_raft_is_closed_and_wound_outwards() {
    let styles = [RaftStyle::Skates, RaftStyle::Grid, RaftStyle::Connected];
    for style in styles {
        for shape in [RaftShape::Hull, RaftShape::Rectangle] {
            let profile = with_raft(style, shape);
            let trees = standing(&TRIANGLE, 20.0, &profile);
            let model = box_mesh(Vec3::new(0.0, 0.0, 22.0), Vec3::new(40.0, 40.0, 30.0));
            let slab = built(&trees, &model, &profile).expect("three feet make a raft");

            let diagnostics = diagnose(&slab);
            assert_eq!(
                diagnostics.boundary_edges, 0,
                "{style:?} {shape:?} left it open"
            );
            assert_eq!(diagnostics.non_manifold_edges, 0, "{style:?} {shape:?}");
            assert_eq!(diagnostics.degenerate_faces, 0, "{style:?} {shape:?}");
            assert!(signed_volume(&slab) > 0.0, "{style:?} {shape:?}");
        }
    }
}

#[test]
fn a_raft_is_never_smaller_than_the_part_above_it() {
    for shape in [RaftShape::Hull, RaftShape::Rectangle] {
        let profile = with_raft(RaftStyle::Skates, shape);
        let trees = standing(&[(20.0, 15.0)], 20.0, &profile);
        let model = box_mesh(Vec3::new(0.0, 0.0, 22.0), Vec3::new(40.0, 30.0, 30.0));
        let slab = built(&trees, &model, &profile).expect("one foot makes a raft");

        let bounds = slab.aabb().expect("the slab has vertices");
        let margin = profile.raft.margin_mm;
        assert!(
            bounds.mins.x <= -margin + 1e-3 && bounds.maxs.x >= 40.0 + margin - 1e-3,
            "{shape:?} reaches the margin past the part either side, got {bounds:?}"
        );
        assert!(bounds.mins.y <= -margin + 1e-3 && bounds.maxs.y >= 30.0 + margin - 1e-3);
        assert!(bounds.mins.z.abs() < 1e-4, "it sits on the plate");
    }
}

#[test]
fn a_hull_raft_goes_out_by_a_margin_smaller_than_its_corners() {
    let mut profile = with_raft(RaftStyle::Skates, RaftShape::Hull);
    profile.raft.margin_mm = 0.5;
    let trees = standing(&[(20.0, 15.0)], 20.0, &profile);
    let model = box_mesh(Vec3::new(0.0, 0.0, 22.0), Vec3::new(40.0, 30.0, 30.0));
    let slab = built(&trees, &model, &profile).expect("one foot makes a raft");

    // The lip flares out past the outline by the slab's overhang.
    let reach = 0.5 + profile.raft.overhang_mm();
    let bounds = slab.aabb().expect("the slab has vertices");
    assert!(
        (bounds.maxs.x - 40.0 - reach).abs() < 1e-2 && (bounds.mins.y + reach).abs() < 1e-2,
        "a straight side of the 40 x 30 mm part is {reach} mm past it, got {bounds:?}"
    );
}

#[test]
fn a_rectangle_raft_turns_with_the_part() {
    let profile = with_raft(RaftStyle::Skates, RaftShape::Rectangle);
    let trees = standing(&[(0.0, 0.0)], 20.0, &profile);
    let turned = Transform::from_scale_rotation_translation(
        Vec3::ONE,
        core_geometry::Quat::from_rotation_z(std::f32::consts::FRAC_PI_4),
        Vec3::ZERO,
    );
    let model = box_mesh(Vec3::new(-20.0, -20.0, 22.0), Vec3::new(20.0, 20.0, 30.0));
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, turned);
    let slab = raft(&trees, &placed, Profiles::single(&profile)).expect("a raft");

    // A 40 mm square turned 45 degrees lies within |x| + |y| <= 20 sqrt 2: its own
    // rectangle, margin and lip's flare added, still does; the plate's would not.
    let reach = 20.0 + profile.raft.margin_mm + 2.0 * profile.raft.overhang_mm();
    let limit = reach * std::f32::consts::SQRT_2 + 1e-3;
    for vertex in &slab.vertices {
        assert!(
            vertex.x.abs() + vertex.y.abs() <= limit,
            "the rectangle turns with the square, {vertex} lies past {limit}"
        );
    }
}

#[test]
fn a_bigger_margin_makes_a_bigger_raft() {
    let small = with_raft(RaftStyle::Skates, RaftShape::Hull);
    let large = SupportProfile {
        raft: Raft {
            margin_mm: 6.0,
            ..small.raft
        },
        ..small.clone()
    };
    let one = built(&standing(&TRIANGLE, 20.0, &small), &Mesh::default(), &small).expect("a raft");
    let other =
        built(&standing(&TRIANGLE, 20.0, &large), &Mesh::default(), &large).expect("a raft");
    assert!(signed_volume(&other) > signed_volume(&one));
}

#[test]
fn a_grid_raft_has_holes_but_never_under_a_foot() {
    let profile = with_raft(RaftStyle::Grid, RaftShape::Rectangle);
    let trees = standing(&[(10.0, 10.0)], 20.0, &profile);
    let model = box_mesh(Vec3::new(0.0, 0.0, 22.0), Vec3::new(60.0, 60.0, 30.0));
    let grid = built(&trees, &model, &profile).expect("a grid");
    let saucer_profile = with_raft(RaftStyle::Skates, RaftShape::Rectangle);
    let saucer = built(&trees, &model, &saucer_profile).expect("a saucer");

    assert!(
        signed_volume(&grid) < 0.9 * signed_volume(&saucer),
        "holes save resin"
    );
    let half = profile.raft.thickness_mm / 2.0;
    assert!(
        inside(&grid, Vec3::new(10.0, 10.0, half)),
        "the foot stands on a pad"
    );
}

#[test]
fn a_skate_raft_is_a_saucer_whose_lip_carries_the_wall_on_up() {
    let profile = with_raft(RaftStyle::Skates, RaftShape::Rectangle);
    let trees = standing(&TRIANGLE, 20.0, &profile);
    let saucer = built(&trees, &Mesh::default(), &profile).expect("a raft");
    let thickness = profile.raft.thickness_mm;
    let bounds = saucer.aabb().expect("vertices");
    assert!(
        (bounds.maxs.z - 2.0 * thickness).abs() < 1e-4,
        "the lip is as tall again as the slab"
    );
    let centre = |z: Scalar| Vec3::new(20.0, 16.0, z);
    assert!(
        inside(&saucer, centre(thickness / 2.0)),
        "a floor under the feet"
    );
    assert!(
        !inside(&saucer, centre(thickness * 1.5)),
        "and nothing over it"
    );

    let width_at = |z: Scalar| {
        let xs = saucer
            .vertices
            .iter()
            .filter(|v| (v.z - z).abs() < 1e-4)
            .map(|v| v.x);
        let (lo, hi) = xs.fold((Scalar::INFINITY, Scalar::NEG_INFINITY), |(lo, hi), x| {
            (lo.min(x), hi.max(x))
        });
        hi - lo
    };
    let flare = (width_at(2.0 * thickness) - width_at(0.0)) / 2.0;
    assert!(
        (flare - 2.0 * profile.raft.overhang_mm()).abs() < 1e-3,
        "the lip leans out at the wall's own slope over twice the height, got {flare}"
    );
    let left_at = |z: Scalar| {
        let xs = saucer.vertices.iter().filter(|v| (v.z - z).abs() < 1e-4);
        xs.map(|v| v.x).fold(Scalar::INFINITY, Scalar::min)
    };
    let (lip_inner, floor_edge) = (left_at(2.0 * thickness) + LIP_WIDTH_MM, left_at(thickness));
    assert!(
        saucer
            .vertices
            .iter()
            .any(|v| (v.z - 2.0 * thickness).abs() < 1e-4 && (v.x - lip_inner).abs() < 1e-3),
        "the lip is {LIP_WIDTH_MM} mm wide across its top"
    );
    let lean = floor_edge - lip_inner;
    assert!(
        (lean - profile.raft.overhang_mm()).abs() < 1e-3,
        "the lip's inner face leans at the wall's slope too, got {lean}"
    );
}

#[test]
fn a_connected_raft_joins_every_foot_however_far() {
    let profile = with_raft(RaftStyle::Connected, RaftShape::Hull);
    let trees = standing(&[(0.0, 0.0), (60.0, 0.0), (60.0, 40.0)], 20.0, &profile);
    let half = profile.raft.thickness_mm / 2.0;
    let tied = built(&trees, &Mesh::default(), &profile).expect("a raft");
    assert!(
        inside(&tied, Vec3::new(30.0, 0.0, half)),
        "60 mm apart and still joined"
    );
    assert!(inside(&tied, Vec3::new(60.0, 20.0, half)));
    assert!(
        !inside(&tied, Vec3::new(30.0, 20.0, half)),
        "the long way round is left out"
    );
}

#[test]
fn the_spanning_tree_takes_the_shortest_way_through_every_point() {
    let points = [
        Vec2::ZERO,
        Vec2::new(10.0, 0.0),
        Vec2::new(5.0, 0.5),
        Vec2::new(5.0, 9.0),
    ];
    let mut edges: Vec<(usize, usize)> = spanning_tree(&points)
        .into_iter()
        .map(|(a, b)| (a.min(b), a.max(b)))
        .collect();
    edges.sort_unstable();
    assert_eq!(edges, vec![(0, 2), (1, 2), (2, 3)]);
}

#[test]
fn a_support_that_stands_on_the_model_needs_no_raft() {
    let profile = with_raft(RaftStyle::Skates, RaftShape::Hull);
    let shelf = box_mesh(Vec3::ZERO, Vec3::splat(10.0));
    let trees = trees_on(
        &[SupportPoint::new(Vec3::new(5.0, 5.0, 20.0))],
        &shelf,
        Transform::default(),
        Profiles::single(&profile),
    );
    assert!(trees[0].landing().on_model);
    assert!(
        built(&trees, &shelf, &profile).is_none(),
        "nothing reaches the plate"
    );
}

#[test]
fn a_raft_reaches_past_the_flare_of_every_group_on_it() {
    let ground = with_raft(RaftStyle::Skates, RaftShape::Hull);
    let mut wide = ground.clone();
    wide.bottom.lower_diameter_mm = 2.5 * ground.bottom.platform_diameter_mm;
    let table = [ground.clone(), wide.clone()];
    let profiles = Profiles::new(&table).expect("two groups");
    let points = [
        SupportPoint::new(Vec3::new(0.0, 0.0, 20.0)),
        SupportPoint::new(Vec3::new(30.0, 0.0, 20.0)).in_group(1),
    ];
    let trees = trees_on(&points, &Mesh::default(), Transform::default(), profiles);
    let model = Mesh::default();
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let slab = raft(&trees, &placed, profiles).expect("a raft");

    let bounds = slab.aabb().expect("the slab has vertices");
    let reach = 30.0 + wide.flare_lower_radius_mm() + ground.raft.margin_mm;
    assert!(
        bounds.maxs.x >= reach - 1e-2,
        "the margin is measured from the wider group's own flare, {reach} mm, got {bounds:?}"
    );
}

#[test]
fn a_raft_that_is_switched_off_is_not_built() {
    let profile = profile();
    let trees = standing(&TRIANGLE, 20.0, &profile);
    assert!(built(&trees, &Mesh::default(), &profile).is_none());
}
