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

/// A 2 mm hole, what the cup tests drill.
const HOLE_MM: Scalar = 2.0;

fn vent_of(meshes: &Mesh, at_mm: [f32; 2]) -> Result<CupVent, Unvented> {
    let bvh = Bvh::build(meshes);
    let (plan, panel) = stack(10.0);
    let risks = [Risk {
        at_mm,
        ..risk_of(CUP)
    }];
    let models = [(meshes, &bvh, Transform::default())];
    cup_vents(&risks, &plan, &panel, &models, HOLE_MM)[0]
}

/// Walls `thick_mm` thick round a 6 mm cavity on the plate, 1 mm thinner at x = 0, 10 mm
/// tall; and the cavity's middle.
fn walls(thick_mm: Scalar) -> (Mesh, [f32; 2]) {
    let span = 2.0 * thick_mm + 5.0;
    let mut mesh = cuboid(Vec3::ZERO, Vec3::new(thick_mm - 1.0, span, 10.0));
    for (from, to) in [
        (
            Vec3::new(span - thick_mm, 0.0, 0.0),
            Vec3::new(span, span, 10.0),
        ),
        (Vec3::ZERO, Vec3::new(span, thick_mm, 10.0)),
        (
            Vec3::new(0.0, span - thick_mm, 0.0),
            Vec3::new(span, span, 10.0),
        ),
    ] {
        mesh.append(&cuboid(from, to));
    }
    let middle = (thick_mm - 1.0 + span - thick_mm) / 2.0;
    (mesh, [middle, span / 2.0])
}

#[test]
fn a_cup_is_vented_down_through_a_floor_off_the_plate() {
    let floor = cuboid(Vec3::new(0.0, 0.0, 3.0), Vec3::new(10.0, 10.0, 5.0));
    let vent = vent_of(&floor, [5.0, 5.0]).expect("the floor stands off the plate");
    assert_eq!(vent.model, 0);
    assert!(
        (vent.mouth_mm.z - 3.0).abs() < 1e-3 && vent.outward == -Vec3::Z,
        "the mouth is on the underside at 3 mm, drilled straight up, got {vent:?}"
    );
    assert!(
        (vent.depth_mm - 3.0).abs() < 1e-2,
        "through 2 mm of floor and 1 mm past it, got {}",
        vent.depth_mm
    );
}

#[test]
fn a_cup_on_the_plate_is_vented_level_through_its_thinnest_wall() {
    let (mesh, middle) = walls(2.0);
    let vent = vent_of(&mesh, middle).expect("a 1 mm wall at x = 0");
    assert!(
        vent.outward.abs_diff_eq(-Vec3::X, 1e-5) && vent.mouth_mm.x.abs() < 1e-3,
        "out through the 1 mm wall at x = 0, got {vent:?}"
    );
    assert!(
        (vent.depth_mm - 2.0).abs() < 1e-2,
        "through 1 mm of wall and 1 mm past it, got {}",
        vent.depth_mm
    );
    assert!(
        (vent.mouth_mm.z - HOLE_MM / 4.0).abs() < 1e-3,
        "a notch along the plate, half the hole under it, got {}",
        vent.mouth_mm.z
    );
}

#[test]
fn a_cup_on_the_plate_in_walls_too_thick_to_drill_is_lifted_or_turned() {
    let (mesh, middle) = walls(17.0);
    assert_eq!(
        vent_of(&mesh, middle),
        Err(Unvented::OnThePlate),
        "16 mm at the thinnest"
    );
}

#[test]
fn a_cup_over_a_floor_lying_on_the_plate_is_lifted_or_turned() {
    let on_plate = cuboid(Vec3::ZERO, Vec3::new(10.0, 10.0, 2.0));
    assert_eq!(vent_of(&on_plate, [5.0, 5.0]), Err(Unvented::OnThePlate));
}

#[test]
fn a_cup_whose_middle_lies_in_the_material_is_left_to_the_hand() {
    let solid = cuboid(Vec3::ZERO, Vec3::splat(10.0));
    assert_eq!(vent_of(&solid, [5.0, 5.0]), Err(Unvented::TooDeep));
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
