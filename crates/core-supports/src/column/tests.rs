use super::*;
use crate::fixtures::{box_mesh, columns_on, landing_on, profile, ramp, under_a_gap_over_a_bar};
use core_geometry::{Bvh, Mesh, Quat, Transform};

/// A wall filling 0..1 on x, standing on the plate.
fn wall() -> Mesh {
    box_mesh(Vec3::new(0.0, -5.0, 0.0), Vec3::new(1.0, 5.0, 20.0))
}

/// A cube spanning 0..10 on every axis, standing on the plate.
fn cube_on_plate() -> Mesh {
    box_mesh(Vec3::ZERO, Vec3::splat(10.0))
}

#[test]
fn a_column_over_nothing_reaches_the_plate() {
    let landing = landing_on(
        &Mesh::default(),
        Transform::default(),
        Vec3::new(3.0, 4.0, 20.0),
        &profile(),
    )
    .expect("20 mm of clearance is enough for a column");

    assert!(!landing.on_model);
    assert!(landing.base.abs_diff_eq(Vec3::new(3.0, 4.0, 0.0), 1e-6));
}

#[test]
fn a_column_over_a_blocked_face_has_nowhere_to_land() {
    let model = cube_on_plate();
    let bvh = Bvh::build(&model);
    let mut region = core_geometry::Region::default();
    for face in 0..model.faces.len() {
        region.set(face, true);
    }
    let blocked =
        crate::Blocked::new(&model, &region, Transform::default()).expect("a painted cube");
    let placed = Placed::new(&model, &bvh, Transform::default()).blocking(Some(&blocked));

    assert!(
        landing(
            &placed,
            Vec3::new(5.0, 5.0, 20.0),
            profile().pillar_radius_mm(),
            1,
            &profile()
        )
        .is_none(),
        "the only surface under the contact is painted out of bounds"
    );
}

#[test]
fn a_column_over_the_model_has_nowhere_to_land_when_the_profile_refuses_the_part() {
    let mut profile = profile();
    profile.land_on_model = false;
    assert!(
        landing_on(
            &cube_on_plate(),
            Transform::default(),
            Vec3::new(5.0, 5.0, 20.0),
            &profile,
        )
        .is_none(),
        "the only surface under the contact is the part itself"
    );
}

#[test]
fn a_column_over_the_model_lands_on_it() {
    let landing = landing_on(
        &cube_on_plate(),
        Transform::default(),
        Vec3::new(5.0, 5.0, 20.0),
        &profile(),
    )
    .expect("10 mm above the cube is enough for a column");

    assert!(landing.on_model);
    assert!(
        (landing.base.z - 10.0).abs() < 1e-4,
        "the cube's top face is at z = 10, got {}",
        landing.base.z
    );
}

#[test]
fn a_column_landing_on_the_model_sinks_into_it() {
    let profile = profile();
    let landing = landing_on(
        &cube_on_plate(),
        Transform::default(),
        Vec3::new(5.0, 5.0, 20.0),
        &profile,
    )
    .expect("there is room");

    let expected = 10.0 - profile.tip.contact_depth_mm;
    assert!(
        (landing.pillar_bottom_z(&profile) - expected).abs() < 1e-4,
        "the pillar must end {} mm inside the surface",
        profile.tip.contact_depth_mm
    );
    assert!((landing.bottom_z(&profile) - expected).abs() < 1e-4);
}

/// A slab spanning 10..12 in z, hanging in the air, and the column of a contact
/// `under_mm` below its underside built to `profile`.
fn column_under_a_slab(under_mm: Scalar, profile: &SupportProfile) -> Column {
    let model = box_mesh(Vec3::new(-5.0, -5.0, 10.0), Vec3::new(5.0, 5.0, 12.0));
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    column(&placed, 0, 0, Vec3::new(0.0, 0.0, 10.0 - under_mm), profile)
        .expect("straight down from the underside of a slab is open air")
}

#[test]
fn a_contact_without_a_bite_a_layer_under_the_surface_is_put_on_it() {
    let mut flush = profile();
    flush.tip.contact_depth_mm = 0.0;
    let column = column_under_a_slab(0.05, &flush);
    assert!(
        (column.contact.z - 10.0).abs() < 1e-4,
        "the slab's underside is at z = 10, got {}",
        column.contact.z
    );
}

#[test]
fn a_contact_that_bites_stays_where_it_was_put() {
    let column = column_under_a_slab(0.05, &profile());
    assert!((column.contact.z - 9.95).abs() < 1e-4);
}

#[test]
fn a_contact_without_a_bite_well_under_the_surface_stays_where_it_was_put() {
    let mut flush = profile();
    flush.tip.contact_depth_mm = 0.0;
    let column = column_under_a_slab(3.0, &flush);
    assert!(
        (column.contact.z - 7.0).abs() < 1e-4,
        "3 mm of air is a contact off the surface, not one a layer short of it"
    );
}

#[test]
fn a_contact_inside_the_part_is_put_on_the_underside_under_it() {
    let mut flush = profile();
    flush.tip.contact_depth_mm = 0.0;
    for profile in [&flush, &profile()] {
        let column = column_under_a_slab(-1.0, profile);
        assert!(
            (column.contact.z - 10.0).abs() < 1e-4,
            "1 mm inside a slab whose underside is at z = 10, got {}",
            column.contact.z
        );
    }
}

#[test]
fn a_column_on_the_plate_stands_on_its_foot() {
    let profile = profile();
    let landing = landing_on(
        &Mesh::default(),
        Transform::default(),
        Vec3::new(1.0, 1.0, 20.0),
        &profile,
    )
    .expect("there is room");

    assert!(
        (landing.pillar_bottom_z(&profile) - profile.bottom.platform_thickness_mm).abs() < 1e-6
    );
    assert!(
        landing.bottom_z(&profile).abs() < 1e-6,
        "the foot is on z = 0"
    );
}

#[test]
fn the_light_preset_stands_a_support_under_a_part_at_its_own_lift() {
    let profile = SupportProfile::shipped("light").expect("the light preset ships");
    let contact = Vec3::new(1.0, 1.0, profile.z_lift_mm);
    assert!(
        landing_on(&Mesh::default(), Transform::default(), contact, &profile).is_some(),
        "the pad, the flare and the top segment have to stand inside the {} mm this \
             profile lifts a part by",
        profile.z_lift_mm
    );
}

#[test]
fn a_contact_with_no_room_under_it_has_no_landing() {
    let profile = profile();
    // The foot alone is taller than this contact stands.
    let contact = Vec3::new(1.0, 1.0, profile.bottom.platform_thickness_mm);
    assert!(landing_on(&Mesh::default(), Transform::default(), contact, &profile).is_none());
}

#[test]
fn the_ray_does_not_land_on_the_face_it_started_from() {
    // A box hanging 10 mm over the plate with the contact on its underside: a ray from the contact
    // would hit that face at zero distance and report a column of no height on the model.
    let hanging = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(10.0, 10.0, 20.0));
    let landing = landing_on(
        &hanging,
        Transform::default(),
        Vec3::new(5.0, 5.0, 10.0),
        &profile(),
    )
    .expect("10 mm of clear air under the contact is room for a column");

    assert!(!landing.on_model, "there is nothing under the overhang");
    assert!((landing.base.z - 0.0).abs() < 1e-6);
}

#[test]
fn a_column_that_would_run_down_the_side_of_the_model_is_not_built() {
    // A 1.2 mm body with half a millimetre of air round it needs 1.1 mm between its
    // axis and the wall's face at x = 1. This one has half of that.
    let contact = Vec3::new(1.5, 0.0, 20.0);
    assert!(landing_on(&wall(), Transform::default(), contact, &profile()).is_none());
}

#[test]
fn a_column_with_room_beside_the_model_still_stands() {
    let contact = Vec3::new(3.0, 0.0, 20.0);
    let landing = landing_on(&wall(), Transform::default(), contact, &profile())
        .expect("2 mm of air beside the wall is room for a 1.2 mm body");
    assert!(!landing.on_model, "it went past the wall to the plate");
}

#[test]
fn a_column_will_not_stand_on_a_face_that_leans_too_far() {
    // A face rising 10 mm over 4 leans 68 degrees from horizontal: a foot on it has
    // nothing under it, and the body would run down the face rather than onto it.
    let steep = ramp(4.0, 10.0);
    let contact = Vec3::new(2.0, 0.0, 20.0);
    assert!(landing_on(&steep, Transform::default(), contact, &profile()).is_none());
}

#[test]
fn a_column_stands_on_a_face_that_leans_within_the_limit() {
    // The same ramp the other way up: 4 mm over 10 is 22 degrees.
    let gentle = ramp(10.0, 4.0);
    let contact = Vec3::new(5.0, 0.0, 20.0);
    let landing = landing_on(&gentle, Transform::default(), contact, &profile())
        .expect("a 22 degree face is something to stand on");
    assert!(landing.on_model);
    assert!(
        (landing.base.z - 2.0).abs() < 1e-3,
        "got {}",
        landing.base.z
    );
}

#[test]
fn a_pad_the_model_is_in_the_way_of_is_moved_aside_whole() {
    // A step 2 mm tall whose face is at x = 1. The body comes down 3 mm clear, but the 8 mm pad
    // keeps 0.5 mm of air, so its middle stands 4.5 mm off, 1.5 mm further than the trunk.
    let kerb = box_mesh(Vec3::new(-10.0, -10.0, 0.0), Vec3::new(1.0, 10.0, 2.0));
    let profile = profile();
    let landing = landing_on(
        &kerb,
        Transform::default(),
        Vec3::new(4.0, 0.0, 20.0),
        &profile,
    )
    .expect("the pad has 2.3 mm to move in before the flare leaves its sole");
    assert!(
        landing.pad_centre().x >= 5.5 - 1e-3,
        "the pad stands clear of the kerb, at {}",
        landing.pad_centre()
    );
    assert!(
        landing.pad_offset.length()
            <= profile.base_radius_mm() - profile.flare_lower_radius_mm() + 1e-4,
        "and the flare is still inside it, {} mm off its middle",
        landing.pad_offset.length()
    );

    let clear = landing_on(
        &kerb,
        Transform::default(),
        Vec3::new(6.0, 0.0, 20.0),
        &profile,
    )
    .expect("6 mm out the pad clears the kerb where it is");
    assert_eq!(clear.pad_offset, Vec2::ZERO);
}

#[test]
fn a_pad_with_nowhere_to_move_is_cut_back_on_the_kerbs_side() {
    // The same step with the trunk 1.9 mm off it: the pad would have to move 2.6 mm
    // and has 2.3, but cut back to 1.4 mm on that side it still holds the 1.1 mm flare.
    let kerb = box_mesh(Vec3::new(-10.0, -10.0, 0.0), Vec3::new(1.0, 10.0, 2.0));
    let landing = landing_on(
        &kerb,
        Transform::default(),
        Vec3::new(2.9, 0.0, 20.0),
        &profile(),
    )
    .expect("a cut pad has room");
    let cut = landing.pad_cut.expect("the pad is cut, not moved");
    assert_eq!(landing.pad_offset, Vec2::ZERO);
    assert!(
        (cut.reach_at(std::f32::consts::PI) - 1.4).abs() < 1e-3,
        "it stops its clearance short of the kerb, got {}",
        cut.reach_at(std::f32::consts::PI)
    );
}

#[test]
fn a_toe_standing_inside_a_pad_is_in_its_way() {
    // A toe 1 mm across standing 2.5 mm out from the pad's middle, between its axis and
    // its rim, where no ray down the pad's rim or axis meets it.
    let toe = box_mesh(Vec3::new(2.0, 1.0, 0.0), Vec3::new(3.0, 2.0, 3.0));
    let bvh = Bvh::build(&toe);
    let placed = Placed::new(&toe, &bvh, Transform::default());
    assert!(!foot_clear(&placed, Vec3::ZERO, &profile()));
    assert!(
        foot_clear(&placed, Vec3::new(-10.0, 0.0, 0.0), &profile()),
        "a pad well away from it is clear"
    );
}

#[test]
fn a_pad_with_nowhere_to_go_is_no_landing() {
    // The same step with the trunk 1.3 mm off it: the pad would have to move 3.2 mm,
    // and has 2.3, and cut back it would bare the flare.
    let kerb = box_mesh(Vec3::new(-10.0, -10.0, 0.0), Vec3::new(1.0, 10.0, 2.0));
    assert!(
        landing_on(
            &kerb,
            Transform::default(),
            Vec3::new(2.3, 0.0, 20.0),
            &profile()
        )
        .is_none()
    );
}

#[test]
fn points_are_resolved_through_the_models_placement() {
    let profile = profile();
    let transform = Transform::from_translation(Vec3::new(50.0, 50.0, 0.0));
    let points = [SupportPoint::new(Vec3::new(5.0, 5.0, 20.0))];

    let columns = columns_on(
        &points,
        &cube_on_plate(),
        transform,
        Profiles::single(&profile),
    );
    assert_eq!(columns.len(), 1);
    assert!(
        columns[0]
            .contact
            .abs_diff_eq(Vec3::new(55.0, 55.0, 20.0), 1e-5),
        "the contact follows the model onto the plate"
    );
    assert!(
        columns[0]
            .landing
            .expect("the cube is under the contact")
            .on_model,
        "the cube moved under it too"
    );
}

#[test]
fn a_point_with_nowhere_to_stand_keeps_its_place_and_has_no_landing() {
    // Kept rather than thrown away: `grow` is where a tip that cannot stand on its
    // own is carried to a trunk that can, or finally dropped.
    let points = [
        SupportPoint::new(Vec3::new(1.0, 1.0, 0.05)),
        SupportPoint::new(Vec3::new(2.0, 2.0, 20.0)),
    ];
    let columns = columns_on(
        &points,
        &Mesh::default(),
        Transform::default(),
        Profiles::single(&profile()),
    );

    assert_eq!(columns.len(), 2);
    assert!(columns[0].landing.is_none(), "it stands under its own foot");
    assert!(columns[1].landing.is_some());
    assert_eq!(columns[1].point, 1, "every column knows its point");
}

#[test]
fn a_tip_placed_on_a_face_looking_up_is_turned_down_as_far_as_it_may() {
    let model = ramp(4.0, 10.0);
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());

    // The middle of the sloping face, which looks up and out towards -X.
    let contact = Vec3::new(2.0, 0.0, 5.0);
    let neck = neck_of(&placed, contact, &profile()).expect("open air above the slope");
    let normal = Vec3::new(-10.0, 0.0, 4.0).normalize();
    assert_leans_off(neck - contact, normal);
}

#[test]
fn a_support_placed_on_a_side_wall_still_stands() {
    // A box raised 10 mm off the plate, clicked on its side at x = 10.
    let raised = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(10.0, 10.0, 20.0));
    let trees = crate::fixtures::trees_on(
        &[SupportPoint::new(Vec3::new(10.0, 5.0, 15.0))],
        &raised,
        Transform::default(),
        Profiles::single(&profile()),
    );
    assert_eq!(
        trees.len(),
        1,
        "the click on the wall is held up from the plate"
    );
    assert!(
        trees[0].landing().base.x > 10.0,
        "it stands out beside the wall, not under it"
    );
}

#[test]
fn a_tip_on_a_corner_leaves_the_face_that_looks_down_whichever_face_is_found_first() {
    use core_geometry::Quat;

    // A box tipped five degrees about X and then about Y, so one corner of its
    // underside hangs lowest and both walls meeting there look a little down too.
    let model = box_mesh(Vec3::ZERO, Vec3::splat(10.0));
    let transform = Transform {
        rotation: Quat::from_rotation_y(5.0_f32.to_radians())
            * Quat::from_rotation_x(-5.0_f32.to_radians()),
        translation: Vec3::new(0.0, 0.0, 20.0),
        ..Transform::default()
    };
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, transform);
    let matrix = transform.to_matrix();

    let underside = matrix.transform_vector3(-Vec3::Z);
    for corner in [
        Vec3::ZERO,
        Vec3::new(10.0, 0.0, 0.0),
        Vec3::new(10.0, 10.0, 0.0),
        Vec3::new(0.0, 10.0, 0.0),
    ] {
        let contact = matrix.transform_point3(corner);
        let neck = neck_of(&placed, contact, &profile()).expect("air under the box");
        let along = (neck - contact).normalize();
        assert!(
            along.dot(underside) > 0.999,
            "of the three faces meeting at {corner} the underside looks down most, \
                 and the tip left along {along}"
        );
    }
}

#[test]
fn a_tip_leaves_a_sloping_face_along_its_normal() {
    // The face rises 4 over a run of 10, so once turned over its normal lies at
    // (-4, 0, -10): 22 degrees off straight down.
    let along = tip_under_a_turned_ramp(10.0, 4.0);
    let expected = Vec3::new(-4.0, 0.0, -10.0).normalize();
    assert!(
        along.normalize().abs_diff_eq(expected, 1e-4),
        "the tip leaves the face square on, got {}",
        along.normalize()
    );
    assert!(
        (along.length() - profile().top.length_mm).abs() < 1e-4,
        "the tip is one top segment long"
    );
}

#[test]
fn a_tip_under_a_face_near_vertical_is_turned_down_to_the_steepest_square_head() {
    // Rising 10 over a run of 4: the normal is 68 degrees off straight down, so the
    // head is turned 23 degrees, within the most it may, to 45.
    let along = tip_under_a_turned_ramp(4.0, 10.0);
    assert_leans_off(along, Vec3::new(-10.0, 0.0, -4.0).normalize());
}

/// Asserts `along` leaves `normal` turned back to [`STEEPEST_SQUARE_HEAD_DEG`] off straight
/// down, by no more than [`MAX_HEAD_LEAN_DEG`].
fn assert_leans_off(along: Vec3, normal: Vec3) {
    let off = normal.dot(-Vec3::Z).acos();
    let turned = (off - MAX_HEAD_LEAN_DEG.to_radians()).max(STEEPEST_SQUARE_HEAD_DEG.to_radians());
    let expected = Vec3::new(normal.x.signum() * turned.sin(), 0.0, -turned.cos());
    assert!(
        along.normalize().abs_diff_eq(expected, 1e-4),
        "the head off {normal} is turned down to {expected}, got {}",
        along.normalize()
    );
}

/// From the contact to the neck of a tip in the middle of the sloping face of
/// `ramp(run, rise)` turned over, so that the face is an overhang.
fn tip_under_a_turned_ramp(run: Scalar, rise: Scalar) -> Vec3 {
    use core_geometry::Quat;
    use std::f32::consts::PI;

    let model = ramp(run, rise);
    let transform = Transform {
        rotation: Quat::from_rotation_x(PI),
        translation: Vec3::new(0.0, 0.0, 30.0),
        ..Transform::default()
    };
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, transform);
    let contact = transform
        .to_matrix()
        .transform_point3(Vec3::new(run / 2.0, 0.0, rise / 2.0));
    neck_of(&placed, contact, &profile()).expect("open air under the turned ramp") - contact
}

#[test]
fn a_trunk_stands_on_a_steep_face_only_when_asked_as_a_last_resort() {
    // A ramp rising 10 over a run of 4: 68 degrees, past what a column chooses to
    // stand on and short of a wall.
    let model = ramp(4.0, 10.0);
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let from = Vec3::new(2.0, 0.0, 20.0);
    let radius_mm = profile().pillar_radius_mm();

    assert!(landing(&placed, from, radius_mm, 1, &profile()).is_none());
    let steep = landing_within(
        &placed,
        from,
        radius_mm,
        1,
        &profile(),
        LAST_RESORT_SLOPE_DEG,
    )
    .expect("there is a face under it, and nowhere better was the caller's to say");
    assert!(steep.on_model);
}

#[test]
fn a_head_that_would_run_into_a_wall_across_from_it_is_not_placed() {
    // A contact on the side of one wall, with another standing a millimetre across.
    let mut model = box_mesh(Vec3::new(0.0, -5.0, 0.0), Vec3::new(1.0, 5.0, 20.0));
    let across = box_mesh(Vec3::new(2.0, -5.0, 0.0), Vec3::new(3.0, 5.0, 20.0));
    let offset = model.vertices.len() as u32;
    model.vertices.extend_from_slice(&across.vertices);
    model
        .faces
        .extend(across.faces.iter().map(|face| face.map(|at| at + offset)));
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());

    let contact = Vec3::new(1.0, 0.0, 10.0);
    let mut fixture = profile();
    fixture.small_pillar.enabled = false;
    assert_eq!(
        neck_of(&placed, contact, &fixture),
        None,
        "turned away, the head would meet its wall slantwise"
    );
    assert_eq!(
        neck_of(&placed, contact, &profile()),
        Some(contact),
        "with small pillars the contact is left headless for one to hold"
    );
}

/// A slab's underside at z = 10, and a lip 1.5 mm deep hanging from it at x = 10.
fn slab_with_lip() -> Mesh {
    let mut model = box_mesh(Vec3::new(0.0, -5.0, 10.0), Vec3::new(10.0, 5.0, 12.0));
    model.append(&box_mesh(
        Vec3::new(10.0, -5.0, 8.5),
        Vec3::new(11.0, 5.0, 12.0),
    ));
    model
}

#[test]
fn a_head_beside_a_lip_is_not_placed_rather_than_turned() {
    // Straight down from 0.3 mm short of the lip, the widening head runs into it.
    let model = slab_with_lip();
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());

    let mut fixture = profile();
    fixture.small_pillar.enabled = false;
    assert_eq!(neck_of(&placed, Vec3::new(9.7, 0.0, 10.0), &fixture), None);
    let clear = neck_of(&placed, Vec3::new(5.0, 0.0, 10.0), &profile())
        .expect("away from the lip the underside is open");
    assert!(clear.abs_diff_eq(Vec3::new(5.0, 0.0, 10.0 - profile().top.length_mm), 1e-4));
}

#[test]
fn a_contact_with_less_room_under_it_than_a_head_is_a_small_pillar_or_nothing() {
    // An underside at z = 10.5 over a bar 9..10: half a millimetre of air, where a
    // 2 mm head would run through the bar.
    let model = under_a_gap_over_a_bar();
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let contact = Vec3::new(0.0, 0.0, 10.5);

    let neck = neck_of(&placed, contact, &profile()).expect("a small pillar fits");
    assert!(!has_head(contact, neck), "got a head to {neck}");
    let mut plain = profile();
    plain.small_pillar.enabled = false;
    assert_eq!(
        neck_of(&placed, contact, &plain),
        None,
        "a pillar is wider than its tip"
    );
}

#[test]
fn a_tip_on_a_face_that_looks_up_is_not_placed() {
    // Square on a flat top the head rises, and a trunk from its neck would come
    // down on the contact; hung down instead, it would run through the box.
    let model = box_mesh(Vec3::ZERO, Vec3::splat(10.0));
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());

    assert_eq!(
        neck_of(&placed, Vec3::new(5.0, 5.0, 10.0), &profile()),
        None
    );
}

#[test]
fn the_pillar_height_is_the_neck_above_the_foot() {
    let profile = profile();
    let columns = columns_on(
        &[SupportPoint::new(Vec3::new(1.0, 1.0, 20.0))],
        &Mesh::default(),
        Transform::default(),
        Profiles::single(&profile),
    );
    let expected = 20.0 - profile.top.length_mm - profile.bottom.platform_thickness_mm;
    let height = columns[0]
        .pillar_height_mm(&profile)
        .expect("the column stands on the plate");
    assert!((height - expected).abs() < 1e-5);
}

#[test]
fn a_contact_with_no_room_for_a_head_moves_along_its_face_to_where_one_has() {
    // 0.4 mm short of the lip the head, 0.5 mm wide where the lip ends, runs into it; from
    // 0.15 mm further back it clears, which is inside the reach a contact may move.
    let model = slab_with_lip();
    let contact = Vec3::new(9.6, 0.0, 10.0);
    let columns = columns_on(
        &[SupportPoint::new(contact)],
        &model,
        Transform::default(),
        Profiles::single(&profile()),
    );
    let column = columns.first().expect("the contact is kept");
    assert!(
        has_head(column.contact, column.neck),
        "it stands on a head, from {}",
        column.contact
    );
    let moved = column.contact - contact;
    assert!(
        moved.length() <= crate::islands::MOVE_REACH_MM + 1e-4 && moved.z.abs() < 1e-3,
        "along the underside and no further than the reach, moved by {moved}"
    );
    assert!(
        column.contact.x < contact.x,
        "away from the lip, to {}",
        column.contact
    );
}

/// [`ramp`] turned over and lifted, so its sloping face looks down and out along -x from
/// z = 20 at x = 0 to z = 10 at x = 4, between its sides at y = ±5.
fn ramp_overhead() -> (Mesh, Transform) {
    let turned = Quat::from_rotation_x(std::f32::consts::PI);
    let transform =
        Transform::from_scale_rotation_translation(Vec3::ONE, turned, Vec3::new(0.0, 0.0, 20.0));
    (ramp(4.0, 10.0), transform)
}

#[test]
fn a_tip_on_the_side_edge_of_a_slope_moves_in_until_its_contact_lies_on_the_face() {
    let profile = profile();
    let radius_mm = profile.contact_radius_mm();
    let (model, transform) = ramp_overhead();
    // On the model the slope is z = 2.5 x; turned over, this is plate (2, -4.95, 15), 0.05 mm in.
    let points = [SupportPoint::new(Vec3::new(2.0, 4.95, 5.0))];
    let columns = columns_on(&points, &model, transform, Profiles::single(&profile));

    let contact = columns
        .first()
        .expect("the slope has room for a head")
        .contact;
    let in_mm = 5.0 - contact.y.abs();
    assert!(
        (radius_mm - 1e-3..=radius_mm + 0.05 + 1e-3).contains(&in_mm),
        "a tip half over the side cures onto the wall past it: {in_mm} mm in, expected \
         {radius_mm} mm, the contact's radius"
    );
    let on_face_mm = (contact.z - (20.0 - 2.5 * contact.x)).abs() / Scalar::hypot(1.0, 2.5);
    assert!(on_face_mm < 1e-3, "{on_face_mm} mm off the sloping face");
}

#[test]
fn a_tip_under_a_rib_narrower_than_its_contact_keeps_its_place() {
    let profile = profile();
    let rib = box_mesh(Vec3::new(0.0, -5.0, 10.0), Vec3::new(0.3, 5.0, 12.0));
    let contact = Vec3::new(0.15, 0.0, 10.0);
    let columns = columns_on(
        &[SupportPoint::new(contact)],
        &rib,
        Transform::default(),
        Profiles::single(&profile),
    );

    assert!(
        profile.contact_radius_mm() * 2.0 > 0.3,
        "the rib is narrower than the contact"
    );
    let kept = columns.first().expect("a rib has room for a head").contact;
    assert!(
        kept.abs_diff_eq(contact, 1e-5),
        "nowhere on a 0.3 mm rib holds the whole tip, so the contact stays at {contact}, got {kept}"
    );
}
