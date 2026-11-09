use core_geometry::cuboid;

use super::*;

fn held_by(meshes: &[Mesh], floor: Vec3) -> Option<(usize, Vec3)> {
    let bvhs: Vec<Bvh> = meshes.iter().map(Bvh::build).collect();
    let models: Vec<Hanging> = meshes
        .iter()
        .zip(&bvhs)
        .map(|(mesh, bvh)| (mesh, bvh, Transform::default()))
        .collect();
    island_holds(&[floor], 0.05, &models)[0]
}

#[test]
fn an_island_a_layer_over_its_floor_is_held_by_the_nearest_underside() {
    let below = cuboid(Vec3::new(0.0, 0.0, 5.08), Vec3::new(10.0, 10.0, 10.0));
    let floating = cuboid(Vec3::new(0.0, 0.0, 5.03), Vec3::new(10.0, 10.0, 10.0));
    let (owner, at) = held_by(&[below, floating], Vec3::new(5.0, 5.0, 5.0))
        .expect("the box's floor is inside the layer over the floor");
    assert_eq!(owner, 1, "the lower underside holds the island");
    assert!(
        (at.z - 5.03).abs() < 1e-4,
        "the box's floor at 5.03 mm, found {at}"
    );
}

#[test]
fn an_island_is_found_on_a_placed_model() {
    let unit = cuboid(Vec3::ZERO, Vec3::splat(10.0));
    let bvh = Bvh::build(&unit);
    let up = Transform::from_translation(Vec3::new(0.0, 0.0, 5.03));
    let found = island_holds(&[Vec3::new(5.0, 5.0, 5.0)], 0.05, &[(&unit, &bvh, up)]);
    let (_, at) = found[0].expect("the placed box's floor is 0.08 mm up");
    assert!((at.z - 5.03).abs() < 1e-4, "placed at 5.03 mm, found {at}");
}

#[test]
fn a_surface_more_than_two_layers_up_holds_no_island_there() {
    let floating = cuboid(Vec3::new(0.0, 0.0, 6.0), Vec3::new(10.0, 10.0, 10.0));
    assert_eq!(
        held_by(&[floating], Vec3::new(5.0, 5.0, 5.0)),
        None,
        "1 mm up is another part's floor, not this island's"
    );
}

/// A plan of 0.05 mm layers to `top_mm`, and an unmirrored panel of 0.05 mm pixels, on
/// which a point's panel millimetres are its plate millimetres.
fn stack(top_mm: Scalar) -> (LayerPlan, RasterSettings) {
    let slice = core_slicer::SliceSettings {
        layer_height: 0.05,
        ..core_slicer::SliceSettings::default()
    };
    let panel = RasterSettings {
        width_px: 2000,
        height_px: 2000,
        pitch: core_raster::PixelPitch { x: 0.05, y: 0.05 },
        mirror_x: false,
        mirror_y: false,
        shading: core_raster::Shading::default(),
        grey: core_raster::Grey::default(),
        blur_px: 0,
    };
    (LayerPlan::uniform(&slice, 0.0, top_mm), panel)
}

/// `kind` at the middle of a 10 mm box, on the layer at 8 mm.
fn risk_of(kind: RiskKind) -> Risk {
    Risk {
        layer: 160,
        at_mm: [5.0, 5.0],
        kind,
    }
}

const CUP: RiskKind = RiskKind::SuctionCup {
    area_mm2: 30.0,
    volume_mm3: 30.0,
};
const LEVER: RiskKind = RiskKind::Lever {
    stress_mpa: 9.0,
    lever_mm: 4.0,
    neck_mm2: 1.0,
    force_n: 3.0,
    gyration_mm: 3.0,
};

#[test]
fn a_cup_is_vented_through_the_floor_that_seals_it() {
    let floor = cuboid(Vec3::new(0.0, 0.0, 3.0), Vec3::new(10.0, 10.0, 5.0));
    let bvh = Bvh::build(&floor);
    let (plan, panel) = stack(10.0);
    let risks = [risk_of(CUP)];
    let models = [(&floor, &bvh, Transform::default())];
    let vent =
        cup_vents(&risks, &plan, &panel, &models)[0].expect("the floor stands off the plate");
    assert_eq!(vent.model, 0);
    assert!(
        (vent.mouth_mm.z - 3.0).abs() < 1e-3 && vent.normal.z < 0.0,
        "the mouth is on the underside at 3 mm, got {vent:?}"
    );
    assert!(
        (vent.depth_mm - (2.0 + VENT_PAST_FLOOR_MM)).abs() < 1e-2,
        "through 2 mm of floor and {VENT_PAST_FLOOR_MM} mm past it, got {}",
        vent.depth_mm
    );
}

#[test]
fn a_cup_sealed_by_the_plate_has_no_vent() {
    let on_plate = cuboid(Vec3::ZERO, Vec3::new(10.0, 10.0, 2.0));
    let bvh = Bvh::build(&on_plate);
    let (plan, panel) = stack(10.0);
    let risks = [risk_of(CUP)];
    let models = [(&on_plate, &bvh, Transform::default())];
    assert_eq!(cup_vents(&risks, &plan, &panel, &models), vec![None]);
}

#[test]
fn a_weak_neck_is_held_at_the_underside_below_the_piece_pulling_on_it() {
    let arm = cuboid(Vec3::new(0.0, 0.0, 4.0), Vec3::new(10.0, 10.0, 9.0));
    let bvh = Bvh::build(&arm);
    let (plan, panel) = stack(10.0);
    let risks = [risk_of(LEVER)];
    let models = [(&arm, &bvh, Transform::default())];
    let holds = lever_holds(&risks, &plan, &panel, &models, &tips());
    let (model, at) = *holds[0].first().expect("an underside at 4 mm");
    assert_eq!(model, 0);
    assert!((at.z - 4.0).abs() < 1e-3, "the underside at 4 mm, got {at}");
}

#[test]
fn a_piece_whose_underside_lies_on_the_plate_is_not_held() {
    let on_plate = cuboid(Vec3::ZERO, Vec3::new(10.0, 10.0, 9.0));
    let bvh = Bvh::build(&on_plate);
    let (plan, panel) = stack(10.0);
    let risks = [risk_of(LEVER)];
    let models = [(&on_plate, &bvh, Transform::default())];
    let holds = lever_holds(&risks, &plan, &panel, &models, &tips());
    assert_eq!(holds, vec![Vec::new()]);
}

/// Medium tips, 0.6 mm across.
fn tips() -> SupportProfile {
    neck_profile(&SupportProfile::default())
}

#[test]
fn a_heavy_neck_gets_several_supports_spread_under_the_piece() {
    let arm = cuboid(Vec3::new(0.0, 0.0, 4.0), Vec3::new(10.0, 10.0, 9.0));
    let bvh = Bvh::build(&arm);
    let (plan, panel) = stack(10.0);
    let heavy = RiskKind::Lever {
        stress_mpa: 9.0,
        lever_mm: 4.0,
        neck_mm2: 1.0,
        force_n: 8.0,
        gyration_mm: 3.0,
    };
    let models = [(&arm, &bvh, Transform::default())];
    let holds = lever_holds(&[risk_of(heavy)], &plan, &panel, &models, &tips());
    assert!(holds[0].len() > 1, "8 N on 1 mm² wants more than one");
    let (one, other) = (holds[0][0].1, holds[0][1].1);
    assert!(one.distance(other) > 1.0, "{one} and {other} stand apart");
}

#[test]
fn a_piece_off_its_own_centre_is_held_where_it_has_an_underside() {
    let mut mesh = cuboid(Vec3::new(0.0, 0.0, 4.0), Vec3::new(3.5, 10.0, 9.0));
    mesh.append(&cuboid(
        Vec3::new(6.5, 0.0, 4.0),
        Vec3::new(10.0, 10.0, 9.0),
    ));
    let bvh = Bvh::build(&mesh);
    let (plan, panel) = stack(10.0);
    let models = [(&mesh, &bvh, Transform::default())];
    let holds = lever_holds(&[risk_of(LEVER)], &plan, &panel, &models, &tips());
    assert!(!holds[0].is_empty(), "the centre is air, the sides are not");
}

#[test]
fn a_tip_thinner_than_a_neck_can_use_is_made_thick_enough() {
    let mut thin = SupportProfile::default();
    thin.tip.contact_diameter_mm = 0.3;
    thin.small_pillar.diameter_mm = 0.3;
    let sturdy = neck_profile(&thin);
    assert!((sturdy.tip.contact_diameter_mm - NECK_TIP_MM).abs() < 1e-6);
    assert!((sturdy.small_pillar.diameter_mm - NECK_TIP_MM).abs() < 1e-6);
}
