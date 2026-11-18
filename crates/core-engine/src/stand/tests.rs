use super::*;

fn medium() -> SupportPlan {
    SupportPlan::of(SupportProfile::shipped("medium").expect("the medium preset ships"))
}

/// `mesh` with no cavity, and `cuts` taken out of it.
fn solid<'a>(mesh: &'a mut Mesh, cuts: &'a mut Option<Mesh>) -> ToStand<'a> {
    ToStand {
        mesh,
        cavity: None,
        on_its_model: false,
        cuts,
        drill_exactly: true,
    }
}

#[test]
fn a_model_with_no_vertices_has_nothing_to_stand_under() {
    let error = stand_under(solid(&mut Mesh::default(), &mut None), &medium(), 0.05)
        .expect_err("an empty mesh cannot be supported");
    assert!(matches!(error, EngineError::EmptyPlate));
}

#[test]
fn a_layer_of_no_height_cannot_be_cut_for_contacts() {
    let mut slab = Mesh::new(
        vec![
            Vec3::new(0.0, 0.0, 10.0),
            Vec3::new(10.0, 0.0, 10.0),
            Vec3::new(0.0, 10.0, 10.0),
            Vec3::new(0.0, 0.0, 12.0),
        ],
        vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
    );
    let error = stand_under(solid(&mut slab, &mut None), &medium(), 0.0)
        .expect_err("no layer height, no stack");
    assert!(matches!(error, EngineError::Supports(_)));
}

/// Axis-aligned box from `low` to `high`, wound outward.
fn cuboid(mesh: &mut Mesh, low: Vec3, high: Vec3) {
    let corner = |at: usize| {
        Vec3::new(
            if at & 1 == 0 { low.x } else { high.x },
            if at & 2 == 0 { low.y } else { high.y },
            if at & 4 == 0 { low.z } else { high.z },
        )
    };
    let box_mesh = Mesh::new(
        (0..8).map(corner).collect(),
        vec![
            [0, 2, 1],
            [1, 2, 3],
            [4, 5, 6],
            [5, 7, 6],
            [0, 1, 4],
            [1, 5, 4],
            [2, 6, 3],
            [3, 6, 7],
            [0, 4, 2],
            [2, 4, 6],
            [1, 3, 5],
            [3, 7, 5],
        ],
    );
    append(mesh, &box_mesh);
}

/// A 10 mm slab on a 2 mm stem standing on the plate: its underside hangs.
fn mushroom() -> Mesh {
    let mut mesh = Mesh::default();
    cuboid(
        &mut mesh,
        Vec3::new(4.0, 4.0, 0.0),
        Vec3::new(6.0, 6.0, 11.0),
    );
    cuboid(
        &mut mesh,
        Vec3::new(0.0, 0.0, 10.0),
        Vec3::new(10.0, 10.0, 12.0),
    );
    mesh
}

#[test]
fn a_seated_model_with_supports_stands_on_its_rafts_top() {
    let mut profile = SupportProfile::shipped("medium").expect("the medium preset ships");
    profile.raft.enabled = true;
    let mut mesh = mushroom();
    let stood = stand_seated(
        solid(&mut mesh, &mut None),
        &SupportPlan::of(profile.clone()),
        0.05,
    )
    .expect("a mushroom on the plate can be supported");
    let low_mm = mesh.aabb().expect("the mushroom has bounds").mins.z;

    assert!(!stood.contacts.is_empty(), "the slab's underside hangs");
    assert!(
        (low_mm - profile.raft.thickness_mm).abs() < 1e-4,
        "the raft is not printed into the stem, got {low_mm}"
    );
    assert!((stood.lifted_mm - profile.raft.thickness_mm).abs() < 1e-4);
}

#[test]
fn contacts_over_a_body_that_may_not_be_stood_on_are_named_unheld() {
    let mut profile = SupportProfile::shipped("medium").expect("the medium preset ships");
    profile.land_on_model = false;
    // A 10 mm slab 2 mm over the middle of a 40 mm block: under it there is only block.
    let mut mesh = Mesh::default();
    cuboid(&mut mesh, Vec3::ZERO, Vec3::new(40.0, 40.0, 5.0));
    cuboid(
        &mut mesh,
        Vec3::new(15.0, 15.0, 7.0),
        Vec3::new(25.0, 25.0, 9.0),
    );
    let stood = stand_under(solid(&mut mesh, &mut None), &SupportPlan::of(profile), 0.05)
        .expect("a block and a slab can be supported");

    assert!(!stood.unheld.is_empty(), "nothing can hold the slab up");
    assert_eq!(
        stood.unheld.len(),
        stood.contacts.len() - stood.standing,
        "every contact is either held or named"
    );
    for lost in &stood.unheld {
        let at = stood.contacts[lost.point];
        assert!(
            (14.0..=26.0).contains(&at.x) && (14.0..=26.0).contains(&at.y),
            "an unheld contact lies under the slab, 15..25 mm across, found {at}"
        );
    }
}

#[test]
fn a_seated_model_needing_no_supports_stays_on_the_plate_over_a_raft() {
    let mut profile = SupportProfile::shipped("medium").expect("the medium preset ships");
    profile.raft.enabled = true;
    let mut mesh = Mesh::default();
    cuboid(&mut mesh, Vec3::ZERO, Vec3::splat(10.0));
    let stood = stand_seated(
        solid(&mut mesh, &mut None),
        &SupportPlan::of(profile.clone()),
        0.05,
    )
    .expect("a cube on the plate can be supported");

    assert!(stood.contacts.is_empty(), "nothing on a cube hangs");
    assert!(stood.lifted_mm.abs() < 1e-6, "no raft is built, so no lift");
}

#[test]
fn the_planned_lift_is_the_lift_standing_gives() {
    let profile = SupportProfile::shipped("medium").expect("the medium preset ships");
    let mut mesh = core_geometry::cuboid(Vec3::ZERO, Vec3::splat(10.0));
    let planned = planned_lift(&mesh, &medium(), false, 0.05).expect("a cube has bounds");
    let stood = stand_under(solid(&mut mesh, &mut None), &medium(), 0.05)
        .expect("a cube on the plate can be supported");

    assert!(
        (planned.lifted_mm - profile.z_lift_mm).abs() < 1e-4,
        "a cube on the plate goes up the profile's {} mm, planned {}",
        profile.z_lift_mm,
        planned.lifted_mm
    );
    assert!((planned.lifted_mm - stood.lifted_mm).abs() < 1e-4);
    assert!(!planned.resting, "it stands clear of the plate");
}

#[test]
fn a_profile_lifting_nothing_leaves_the_model_resting_on_the_plate() {
    let mut profile = SupportProfile::shipped("medium").expect("the medium preset ships");
    profile.z_lift_mm = 0.0;
    let mesh = core_geometry::cuboid(Vec3::ZERO, Vec3::splat(10.0));
    let planned =
        planned_lift(&mesh, &SupportPlan::of(profile), false, 0.05).expect("a cube has bounds");
    assert_eq!(planned, Lift::ON_THE_PLATE);
}

#[test]
fn a_seated_model_is_planned_onto_its_raft_only_when_something_hangs() {
    let mut profile = SupportProfile::shipped("medium").expect("the medium preset ships");
    profile.raft.enabled = true;
    let plan = SupportPlan::of(profile.clone());
    let mut hanging = mushroom();
    let planned = planned_lift(&hanging, &plan, true, 0.05).expect("a mushroom has bounds");
    let stood = stand_seated(solid(&mut hanging, &mut None), &plan, 0.05)
        .expect("a mushroom on the plate can be supported");
    assert!(
        (planned.lifted_mm - profile.raft.thickness_mm).abs() < 1e-4,
        "up the raft's {} mm, planned {}",
        profile.raft.thickness_mm,
        planned.lifted_mm
    );
    assert!((planned.lifted_mm - stood.lifted_mm).abs() < 1e-4);
    assert!(planned.resting, "on the raft's top");

    let cube = core_geometry::cuboid(Vec3::ZERO, Vec3::splat(10.0));
    let planned = planned_lift(&cube, &plan, true, 0.05).expect("a cube has bounds");
    assert_eq!(planned, Lift::ON_THE_PLATE, "nothing on a cube hangs");
}

#[test]
fn contacts_alone_stand_a_column_each_and_lift_nothing() {
    let mut profile = SupportProfile::shipped("medium").expect("the medium preset ships");
    profile.raft.enabled = false;
    let mut mesh = mushroom();
    let island = Vec3::new(1.0, 1.0, 10.0);
    let stood = stand_at(
        solid(&mut mesh, &mut None),
        &SupportPlan::of(profile),
        &[island],
    )
    .expect("the cap's underside can be held up");
    assert_eq!(stood.lifted_mm, 0.0, "the model stays where it stands");
    assert_eq!(stood.contacts, vec![island], "the island alone");
    assert_eq!(stood.standing, 1, "a column under it: {stood:?}");
}

#[test]
fn a_shell_on_its_model_prints_drilled_with_its_cavity() {
    let model = core_geometry::cuboid(Vec3::ZERO, Vec3::splat(20.0));
    let void = core_geometry::cuboid(Vec3::splat(5.0), Vec3::splat(15.0));
    let offset = model.vertices.len() as u32;
    let mut shell = model.clone();
    shell.vertices.extend(&void.vertices);
    shell.faces.extend(
        void.faces
            .iter()
            .map(|[a, b, c]| [a + offset, c + offset, b + offset]),
    );
    let cavity = model.faces.len()..shell.faces.len();
    let hole = core_volume::DrainHole {
        at: Vec3::new(10.0, 10.0, 20.0),
        axis: Vec3::NEG_Z,
        diameter_mm: 2.0 * std::f32::consts::SQRT_2,
        depth_mm: 8.0,
        taper: 1.0,
        lift_mm: 0.0,
        shape: core_volume::HoleShape::Polygon { sides: 4 },
        turn_deg: 45.0,
        kind: core_volume::HoleKind::Drain,
    };
    let cuts = core_volume::drill(&[hole], &[]).expect("the hole drills");

    let (surface, inner) = printed(&shell, (Some(cavity), true), Some(&cuts), true);
    // 8000 mm3 less the 1000 mm3 cavity and the 2 x 2 x 5 mm bore through its wall.
    let volume = core_geometry::signed_volume(&surface);
    assert!(
        (volume - 6980.0).abs() < 1e-2,
        "expected 6980 mm3, got {volume}"
    );
    assert!(
        inner.is_some_and(|inner| !inner.is_empty()),
        "a cavity stays"
    );
}
