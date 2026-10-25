use super::*;
use core_geometry::{Quat, signed_volume};
use std::f32::consts::FRAC_PI_2;

fn drain_tool(diameter_mm: Scalar, depth_mm: Scalar) -> HoleSize {
    HoleSize::round(diameter_mm, depth_mm, 1.0)
}

/// A flat lid in the z = 0 plane, wide enough to drill anywhere in these tests: a
/// surface that rises nowhere, so a hole on it needs no lift.
fn lid() -> Mesh {
    Mesh::new(
        vec![
            Vec3::new(-50.0, -50.0, 0.0),
            Vec3::new(50.0, -50.0, 0.0),
            Vec3::new(50.0, 50.0, 0.0),
            Vec3::new(-50.0, 50.0, 0.0),
        ],
        vec![[0, 1, 2], [0, 2, 3]],
    )
}

fn shell(shape: Mat3, settings: HollowSettings) -> Shell {
    Shell {
        mesh: Arc::new(Mesh::default()),
        cavity: 0..0,
        cavity_mm3: 1234.0,
        voxel_mm: 0.2,
        coarsened: false,
        shape,
        settings,
    }
}

#[test]
fn a_new_object_is_solid() {
    let hollow = ModelHollow::default();
    assert!(!hollow.is_hollow());
    assert!(hollow.shell().is_none());
    assert!(hollow.cavity_mm3().abs() < f32::EPSILON);
}

#[test]
fn a_blocker_is_stored_in_the_models_own_space() {
    let transform = Transform::from_translation(Vec3::new(50.0, 0.0, 0.0));
    let mut hollow = ModelHollow::default();
    hollow.add_blocker(Vec3::new(60.0, 1.0, 2.0), 4.0, transform);

    let blocker = hollow.blockers()[0];
    assert!(blocker.from.abs_diff_eq(Vec3::new(10.0, 1.0, 2.0), 1e-4));
    assert!((blocker.radius_mm - 4.0).abs() < 1e-4);
}

#[test]
fn a_blocker_on_a_scaled_model_keeps_the_size_it_was_asked_for() {
    let transform = Transform {
        scale: Vec3::splat(2.0),
        ..Transform::default()
    };
    let mut hollow = ModelHollow::default();
    hollow.add_blocker(Vec3::new(4.0, 0.0, 0.0), 4.0, transform);

    // Two millimetres in the model's own space is four on a plate it is doubled on.
    let blocker = hollow.blockers()[0];
    assert!((blocker.radius_mm - 2.0).abs() < 1e-4);
    assert!(blocker.from.abs_diff_eq(Vec3::new(2.0, 0.0, 0.0), 1e-4));
}

#[test]
fn a_blocker_travels_with_the_model_it_was_put_on() {
    let transform = Transform {
        rotation: Quat::from_rotation_z(FRAC_PI_2),
        ..Transform::default()
    };
    let mut hollow = ModelHollow::default();
    hollow.add_blocker(Vec3::new(-10.0, 10.0, 0.0), 4.0, transform);

    // A quarter turn about z maps the model's own (10, 10) onto the plate's (-10, 10).
    assert!(
        hollow.blockers()[0]
            .from
            .abs_diff_eq(Vec3::new(10.0, 10.0, 0.0), 1e-4)
    );
}

#[test]
fn clicking_a_blocker_takes_it_away() {
    let mut hollow = ModelHollow::default();
    hollow.add_blocker(Vec3::ZERO, 4.0, Transform::default());

    assert!(!hollow.remove_blocker(Vec3::new(20.0, 0.0, 0.0), Transform::default()));
    assert!(hollow.remove_blocker(Vec3::new(1.0, 0.0, 0.0), Transform::default()));
    assert!(hollow.blockers().is_empty());
    assert!(
        hollow.markers().is_none(),
        "the marker goes with the blocker"
    );
}

#[test]
fn a_blocker_is_drawn_as_a_ball_where_it_stands() {
    let mut hollow = ModelHollow::default();
    hollow.add_blocker(Vec3::new(3.0, 0.0, 0.0), 2.0, Transform::default());

    let markers = hollow.markers().expect("a blocker is drawn").clone();
    let bounds = markers.aabb().expect("the marker has vertices");
    assert!(bounds.mins.abs_diff_eq(Vec3::new(1.0, -2.0, -2.0), 1e-4));
    assert!(bounds.maxs.abs_diff_eq(Vec3::new(5.0, 2.0, 2.0), 1e-4));
}

#[test]
fn a_drain_hole_is_stored_in_the_models_own_space_and_points_into_it() {
    let transform = Transform::from_translation(Vec3::new(50.0, 0.0, 0.0));
    let mut hollow = ModelHollow::default();
    hollow.add_drain(
        &lid(),
        &Bvh::build(&lid()),
        Vec3::new(60.0, 0.0, 10.0),
        Vec3::Z,
        drain_tool(3.0, 4.0),
        transform,
    );

    let hole = hollow.drains()[0];
    assert!(hole.at.abs_diff_eq(Vec3::new(10.0, 0.0, 10.0), 1e-4));
    assert!(
        hole.axis.abs_diff_eq(Vec3::NEG_Z, 1e-4),
        "a hole is drilled the other way from the face it was clicked on, got {}",
        hole.axis
    );
}

#[test]
fn a_drain_hole_on_a_scaled_model_keeps_the_size_it_was_asked_for() {
    let transform = Transform {
        scale: Vec3::splat(2.0),
        ..Transform::default()
    };
    let mut hollow = ModelHollow::default();
    hollow.add_drain(
        &lid(),
        &Bvh::build(&lid()),
        Vec3::new(0.0, 0.0, 4.0),
        Vec3::Z,
        drain_tool(3.0, 4.0),
        transform,
    );

    let hole = hollow.drains()[0];
    assert!((hole.diameter_mm - 1.5).abs() < 1e-4);
    assert!((hole.depth_mm - 2.0).abs() < 1e-4);
}

#[test]
fn clicking_a_drain_hole_takes_it_away() {
    let mut hollow = ModelHollow::default();
    hollow.add_drain(
        &lid(),
        &Bvh::build(&lid()),
        Vec3::ZERO,
        Vec3::Z,
        drain_tool(4.0, 5.0),
        Transform::default(),
    );

    assert!(!hollow.remove_drain(Vec3::new(9.0, 0.0, 0.0), Transform::default()));
    assert!(hollow.remove_drain(Vec3::new(0.0, 0.0, -2.0), Transform::default()));
    assert!(hollow.drains().is_empty());
    assert!(hollow.cut_bodies().is_none(), "the tube goes with the hole");
}

#[test]
fn a_click_on_a_hole_drills_nothing_more_but_a_wider_one_over_it_is_drilled() {
    let mut hollow = ModelHollow::default();
    let (mesh, bvh) = (lid(), Bvh::build(&lid()));
    let mut click = |at: Vec3, diameter_mm: Scalar| {
        let size = drain_tool(diameter_mm, 5.0);
        hollow.add_drain(&mesh, &bvh, at, Vec3::Z, size, Transform::default());
    };
    click(Vec3::ZERO, 3.0);
    click(Vec3::new(0.5, 0.0, 0.0), 3.0);
    click(Vec3::ZERO, 2.0);
    click(Vec3::ZERO, 10.0);
    assert_eq!(
        hollow.drains().len(),
        2,
        "the same and a narrower hole land in the 3 mm one, the 10 mm one is drilled"
    );
}

#[test]
fn a_narrower_hole_on_the_floor_of_one_already_drilled_is_drilled_through_it() {
    let mut hollow = ModelHollow::default();
    let (mesh, bvh) = (lid(), Bvh::build(&lid()));
    let mut click = |at: Vec3, diameter_mm: Scalar, depth_mm: Scalar| {
        let size = drain_tool(diameter_mm, depth_mm);
        hollow.add_drain(&mesh, &bvh, at, Vec3::Z, size, Transform::default());
    };
    click(Vec3::ZERO, 6.0, 2.0);
    // On the 6 mm hole's floor, 2 mm down, and on to 4 mm.
    click(Vec3::new(0.0, 0.0, -2.0), 3.0, 2.0);
    // At the old mouth and no deeper than its floor: wholly inside it.
    click(Vec3::ZERO, 3.0, 1.0);
    assert_eq!(
        hollow.drains().len(),
        2,
        "the hole reaching past the floor is drilled, the one inside the old hole is not"
    );
}

#[test]
fn a_hole_in_the_supports_is_kept_with_the_model_and_neither_deepened_nor_walled() {
    let raft = core_geometry::cuboid(Vec3::new(-10.0, -10.0, 0.0), Vec3::new(10.0, 10.0, 2.0));
    let bvh = Bvh::build(&raft);
    let placed = Transform::from_translation(Vec3::new(0.0, 0.0, 5.0));
    let mut hollow = ModelHollow::default();
    let top = Vec3::new(1.0, 2.0, 2.0);
    let hole =
        ModelHollow::support_drain_at(&raft, &bvh, top, Vec3::Z, drain_tool(3.0, 1.0), placed)
            .expect("a raft takes a hole");
    hollow.add_hole(hole);

    let hole = hollow.drains()[0];
    assert_eq!(hole.kind, HoleKind::Support);
    assert!(
        hole.at.abs_diff_eq(Vec3::new(1.0, 2.0, -3.0), 1e-5),
        "kept in the model's space, 5 mm under its origin, got {:?}",
        hole.at
    );
    let asked = HollowSettings::default();
    assert!(
        hollow.asking(&asked).blockers.is_empty(),
        "the cavity keeps no wall off it"
    );
    hollow.take(shell(Mat3::IDENTITY, asked));
    assert!(
        (hollow.cut().0[0].depth_mm - 1.0).abs() < 1e-6,
        "a hollow model leaves it at the 1 mm it was asked for"
    );
}

#[test]
fn what_was_drilled_holds_only_for_the_cuts_and_mesh_it_was_drilled_from() {
    let (mesh, bvh) = (Arc::new(lid()), Bvh::build(&lid()));
    let mut hollow = ModelHollow::default();
    let click = |hollow: &mut ModelHollow, x: Scalar| {
        let size = drain_tool(3.0, 5.0);
        let at = Vec3::new(x, 0.0, 0.0);
        hollow.add_drain(&mesh, &bvh, at, Vec3::Z, size, Transform::default());
    };
    click(&mut hollow, 0.0);
    assert!(
        hollow.wants_drilling(&mesh),
        "a new hole waits to be drilled"
    );
    let cuts = Arc::clone(hollow.cut_bodies().expect("the hole is cut"));
    let drilled = Drilled {
        mesh: Arc::clone(&mesh),
        cavity: None,
    };
    hollow.keep_drilled(&mesh, &cuts, Some(drilled));
    assert!(hollow.drilled(&mesh).is_some(), "kept for the same cuts");
    assert!(
        hollow.drilled(&Arc::new(lid())).is_none(),
        "not for a mesh that replaced it"
    );

    click(&mut hollow, 20.0);
    assert!(
        hollow.drilled(&mesh).is_none(),
        "a second hole drills again"
    );
    hollow.keep_drilled(&mesh, &cuts, None);
    assert!(
        hollow.wants_drilling(&mesh),
        "and what was drilled for the first alone is not kept"
    );
}

#[test]
fn a_drain_hole_is_cut_as_the_tube_it_takes_out() {
    let mut hollow = ModelHollow::default();
    hollow.add_drain(
        &lid(),
        &Bvh::build(&lid()),
        Vec3::ZERO,
        Vec3::Z,
        drain_tool(4.0, 5.0),
        Transform::default(),
    );

    let bodies = hollow.cut_bodies().expect("a hole is a body").clone();
    let bounds = bodies.aabb().expect("the tube has vertices");
    assert!(
        bounds.maxs.z > 0.0 && bounds.mins.z <= -5.0,
        "the tube runs from just clear of the lid down to its own depth, got {bounds:?}"
    );
}

#[test]
fn a_channel_is_dug_only_once_it_has_two_points_to_run_between() {
    let mut hollow = ModelHollow::default();
    hollow.add_channel_point(Vec3::new(1.0, 0.0, 0.0), Vec3::X, Transform::default());
    assert!(
        !hollow.finish_channel(HoleSize::round(2.0, 0.0, 1.0), Transform::default()),
        "one point is not a channel"
    );
    assert!(hollow.channels().is_empty());

    hollow.add_channel_point(Vec3::new(1.0, 0.0, 0.0), Vec3::X, Transform::default());
    hollow.add_channel_point(Vec3::new(1.0, 0.0, 6.0), Vec3::X, Transform::default());
    assert!(hollow.finish_channel(HoleSize::round(2.0, 0.0, 1.0), Transform::default()));

    let channel = &hollow.channels()[0];
    assert!((channel.diameter_mm - 2.0).abs() < 1e-4);
    assert_eq!(
        channel.points.len(),
        4,
        "a mouth at each end over the dive under either click"
    );
    for under in &channel.points[1..3] {
        assert!(
            under.x <= 1.0 - 1.0,
            "the run is sunk clear of the face it was drawn on, got {under}"
        );
    }
    assert!(
        hollow.pending().is_empty(),
        "the points go into the channel that was dug from them"
    );
}

#[test]
fn a_hole_is_cut_the_moment_it_is_placed() {
    let mut hollow = ModelHollow::default();
    hollow.add_drain(
        &lid(),
        &Bvh::build(&lid()),
        Vec3::ZERO,
        Vec3::Z,
        drain_tool(3.0, 0.5),
        Transform::default(),
    );

    assert!(
        (hollow.cut().0[0].depth_mm - 0.5).abs() < 1e-6,
        "a hole on a solid model is cut to the depth it was asked for"
    );
    let bodies = hollow
        .cut_bodies()
        .expect("a placed hole is a body")
        .clone();
    assert!(
        signed_volume(&bodies) < 0.0,
        "the body is wound inward, which is what subtracts it"
    );
    assert!(
        hollow.markers().is_none(),
        "a hole is geometry from the first click, never a marker"
    );
}

#[test]
fn a_hole_drilled_before_the_cavity_keeps_its_depth_and_one_after_goes_through_the_wall() {
    let (mesh, size) = (lid(), drain_tool(3.0, 0.5));
    let bvh = Bvh::build(&mesh);
    let mut hollow = ModelHollow::default();
    hollow.add_drain(&mesh, &bvh, Vec3::ZERO, Vec3::Z, size, Transform::default());
    // Two millimetres of wall on a 0.2 mm lattice, which a half-millimetre dimple
    // cannot reach through.
    hollow.take(shell(
        Mat3::IDENTITY,
        hollow.asking(&HollowSettings::default()),
    ));
    let at = Vec3::new(20.0, 0.0, 0.0);
    hollow.add_drain(&mesh, &bvh, at, Vec3::Z, size, Transform::default());

    let (before, after) = (hollow.cut().0[0], hollow.cut().0[1]);
    assert!(
        (before.depth_mm - 0.5).abs() < 1e-6,
        "a hole drilled into the solid model is its shape, still 0.5 mm deep, got {}",
        before.depth_mm
    );
    assert!(
        after.depth_mm >= 2.0,
        "a hole drilled into the shell is re-cut through the 2 mm wall, got {}",
        after.depth_mm
    );
}

#[test]
fn a_hole_drilled_into_a_solid_model_asks_for_a_wall_around_it() {
    let asked = HollowSettings::default();
    let mut hollow = ModelHollow::default();
    hollow.add_drain(
        &lid(),
        &Bvh::build(&lid()),
        Vec3::ZERO,
        Vec3::Z,
        drain_tool(3.0, 4.0),
        Transform::default(),
    );
    let walled = hollow.asking(&asked);
    assert_eq!(
        walled.blockers,
        vec![
            Blocker::rod(Vec3::ZERO, Vec3::new(0.0, 0.0, -4.0), 1.5, 0.0)
                .walled(asked.thickness_mm)
        ],
        "the cavity keeps a wall's width off the hole's whole length"
    );

    hollow.take(shell(Mat3::IDENTITY, walled));
    hollow.clear_drains();
    assert_eq!(
        hollow.rebuild(),
        Some(asked),
        "a walled hole filled in leaves the cavity to be rebuilt without its wall"
    );
}

#[test]
fn the_hole_a_click_would_drill_is_the_hole_it_cuts() {
    let mut hollow = ModelHollow::default();
    hollow.take(shell(
        Mat3::IDENTITY,
        hollow.asking(&HollowSettings::default()),
    ));
    let (mesh, size) = (lid(), drain_tool(3.0, 0.5));
    let bvh = Bvh::build(&mesh);
    let placed = Transform::from_translation(Vec3::new(10.0, 0.0, 0.0));
    let at = Vec3::new(12.0, 1.0, 0.0);

    let would = ModelHollow::drain_at(&mesh, &bvh, at, Vec3::Z, size, placed)
        .expect("a flat lid takes a hole anywhere");
    hollow.add_drain(&mesh, &bvh, at, Vec3::Z, size, placed);
    assert_eq!(
        hollow.as_cut(would),
        hollow.cut().0[0],
        "what the cursor shows is what the click cuts, through the wall"
    );
}

#[test]
fn drilling_a_hole_leaves_the_shell_alone() {
    let asked = HollowSettings::default();
    let mut hollow = ModelHollow::default();
    hollow.take(shell(Mat3::IDENTITY, asked.clone()));
    hollow.add_drain(
        &lid(),
        &Bvh::build(&lid()),
        Vec3::ZERO,
        Vec3::Z,
        drain_tool(3.0, 4.0),
        Transform::default(),
    );

    assert!(
        !hollow.is_stale(&asked, Transform::default()),
        "a hole is cut on its own, so the cavity does not have to be built again"
    );
}

#[test]
fn a_shell_built_from_the_same_numbers_is_not_stale() {
    let asked = HollowSettings::default();
    let mut hollow = ModelHollow::default();
    hollow.take(shell(Mat3::IDENTITY, asked.clone()));

    assert!(!hollow.is_stale(&asked, Transform::default()));
}

#[test]
fn changing_the_wall_makes_the_shell_stale() {
    let mut hollow = ModelHollow::default();
    hollow.take(shell(Mat3::IDENTITY, HollowSettings::default()));

    let thicker = HollowSettings {
        thickness_mm: 3.0,
        ..HollowSettings::default()
    };
    assert!(hollow.is_stale(&thicker, Transform::default()));
}

#[test]
fn adding_a_blocker_makes_the_shell_stale() {
    let asked = HollowSettings::default();
    let mut hollow = ModelHollow::default();
    hollow.take(shell(Mat3::IDENTITY, asked.clone()));
    hollow.add_blocker(Vec3::ZERO, 4.0, Transform::default());

    assert!(hollow.is_stale(&asked, Transform::default()));
}

#[test]
fn digging_a_channel_makes_the_shell_stale_and_asks_for_a_sleeve_of_wall() {
    let asked = HollowSettings::default();
    let mut hollow = ModelHollow::default();
    hollow.take(shell(Mat3::IDENTITY, asked.clone()));
    hollow.add_channel_point(Vec3::new(1.0, 0.0, 0.0), Vec3::X, Transform::default());
    hollow.add_channel_point(Vec3::new(1.0, 0.0, 6.0), Vec3::X, Transform::default());
    hollow.finish_channel(HoleSize::round(2.0, 0.0, 1.0), Transform::default());

    assert!(
        hollow.is_stale(&asked, Transform::default()),
        "the cavity has to be cut again to keep off the pipe"
    );
    let wanted = hollow.asking(&asked);
    assert_eq!(
        wanted.blockers.len(),
        3,
        "a sleeve over each leg of the spine"
    );
    assert!(
        wanted
            .blockers
            .iter()
            .all(|blocker| (blocker.radius_mm - 1.0).abs() < 1e-4
                && (blocker.wall_mm - asked.thickness_mm).abs() < 1e-6),
        "the sleeve is the tube's radius and the wall around it"
    );
}

#[test]
fn a_shell_asks_to_be_rebuilt_once_a_channel_is_dug_or_cleared_and_not_before() {
    let mut hollow = ModelHollow::default();
    assert_eq!(
        hollow.rebuild(),
        None,
        "a solid model has nothing to rebuild"
    );
    let built = hollow.asking(&HollowSettings::for_wall(3.0));
    hollow.take(shell(Mat3::IDENTITY, built));
    assert_eq!(
        hollow.rebuild(),
        None,
        "the shell is what it was built from"
    );

    hollow.add_channel_point(Vec3::new(1.0, 0.0, 0.0), Vec3::X, Transform::default());
    hollow.add_channel_point(Vec3::new(1.0, 0.0, 6.0), Vec3::X, Transform::default());
    hollow.finish_channel(HoleSize::round(2.0, 0.0, 1.0), Transform::default());
    let sleeved = hollow.rebuild().expect("the pipe needs its sleeve");
    assert!(
        (sleeved.thickness_mm - 3.0).abs() < 1e-6,
        "at the wall it was built at, whatever the tool now says"
    );
    assert_eq!(sleeved.blockers.len(), 3, "a sleeve over each leg");

    hollow.take(shell(Mat3::IDENTITY, sleeved));
    hollow.clear_channels();
    assert!(
        hollow
            .rebuild()
            .is_some_and(|wanted| wanted.blockers.is_empty()),
        "a sleeve with no pipe in it is solid resin for nothing"
    );
}

#[test]
fn rescaling_the_model_makes_the_shell_stale() {
    let asked = HollowSettings::default();
    let mut hollow = ModelHollow::default();
    hollow.take(shell(Mat3::IDENTITY, asked.clone()));

    let doubled = Transform {
        scale: Vec3::splat(2.0),
        ..Transform::default()
    };
    assert!(
        hollow.is_stale(&asked, doubled),
        "a wall measured at one scale is the wrong thickness once the model is \
             scaled again"
    );
}

#[test]
fn turning_the_model_keeps_the_shell_and_stretching_it_does_not() {
    let asked = HollowSettings::default();
    let placed = Transform {
        scale: Vec3::new(2.0, 1.0, 1.0),
        ..Transform::default()
    };
    let mut hollow = ModelHollow::default();
    hollow.take(shell(placed.unturned(), asked.clone()));

    let turned = placed.turned(Quat::from_rotation_x(0.8));
    assert!(!hollow.is_stale(&asked, turned), "a turn is no resize");
    let stretched = turned.stretched(Vec3::new(1.0, 1.0, 1.5));
    assert!(
        hollow.is_stale(&asked, stretched),
        "a turned model stretched along the plate has walls of new thicknesses"
    );
}

#[test]
fn cuts_carried_onto_a_pressed_surface_stand_where_they_printed_and_the_model_is_solid() {
    let placed = Transform {
        scale: Vec3::splat(2.0),
        ..Transform::default()
    };
    let mut hollow = ModelHollow::default();
    hollow.add_blocker(Vec3::new(10.0, 0.0, 0.0), 4.0, placed);
    let (lid, bvh) = (lid(), Bvh::build(&lid()));
    hollow.add_drain(
        &lid,
        &bvh,
        Vec3::new(0.0, 0.0, 4.0),
        Vec3::Z,
        drain_tool(3.0, 4.0),
        placed,
    );
    hollow.take(shell(placed.unturned(), HollowSettings::default()));

    let carried = hollow.carried_onto(&lid, &bvh, placed.sizing().linear());

    // With the doubling pressed into the mesh, own space is plate space: the click's
    // point and the sizes asked for on the plate come back as they were.
    let blocker = carried.blockers()[0];
    assert!(blocker.from.abs_diff_eq(Vec3::new(10.0, 0.0, 0.0), 1e-4));
    assert!((blocker.radius_mm - 4.0).abs() < 1e-4, "the 4 mm asked for");
    let hole = carried.drains()[0];
    assert!((hole.diameter_mm - 3.0).abs() < 1e-4, "the 3 mm asked for");
    assert!((hole.depth_mm - 4.0).abs() < 1e-4, "the 4 mm asked for");
    assert!(
        !carried.is_hollow(),
        "the cavity is cut again into the new surface"
    );
    assert!(carried.cut_bodies().is_some(), "the hole is cut at once");
}

#[test]
fn a_support_hole_carried_onto_a_pressed_surface_keeps_the_lift_its_supports_gave_it() {
    let placed = Transform {
        scale: Vec3::splat(2.0),
        ..Transform::default()
    };
    let under_the_lid = Vec3::new(0.0, 0.0, -10.0);
    let mut hollow = ModelHollow::default();
    hollow.add_hole(DrainHole {
        kind: HoleKind::Support,
        ..DrainHole::sized(under_the_lid, Vec3::NEG_Z, drain_tool(3.0, 2.0), 0.25)
    });
    let (lid, bvh) = (lid(), Bvh::build(&lid()));

    let carried = hollow.carried_onto(&lid, &bvh, placed.sizing().linear());

    let lift_mm = carried.drains()[0].lift_mm;
    assert!(
        (lift_mm - 0.5).abs() < 1e-5,
        "the 0.25 mm measured on the supports, doubled, not one measured on the model, got {lift_mm}"
    );
}
