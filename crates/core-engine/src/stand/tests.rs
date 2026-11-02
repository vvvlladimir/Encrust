use super::*;

fn medium() -> SupportPlan {
    SupportPlan::of(SupportProfile::shipped("medium").expect("the medium preset ships"))
}

/// `mesh` with no cavity, and `cuts` taken out of it.
fn solid<'a>(mesh: &'a mut Mesh, cuts: &'a mut Option<Mesh>) -> ToStand<'a> {
    ToStand {
        mesh,
        cavity: None,
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
