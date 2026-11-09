//! What a measured stack shows, answered: where a support stands under an island or a weak
//! neck, and where a hole lets the air into a suction cup. See `docs/design/check-fixes.md`.

use core_analysis::{Risk, RiskKind};
use core_geometry::{
    Bvh, Mat4, Mesh, PlacedHit, Ray, Scalar, Transform, Vec3, raycast_placed_within,
};
use core_raster::RasterSettings;
use core_slicer::LayerPlan;

/// How many layers over an island's floor its underside may first be found.
const UNDERSIDE_LAYERS: Scalar = 2.0;

/// A model with its placement as a matrix and that matrix's inverse.
type Placed<'a> = (&'a Mesh, &'a Bvh, Mat4, Mat4);

fn placed<'a>(models: &[Hanging<'a>]) -> Vec<(usize, Placed<'a>)> {
    models
        .iter()
        .enumerate()
        .filter_map(|(id, (mesh, bvh, transform))| {
            let inverse = transform.inverse_matrix()?;
            Some((id, (*mesh, *bvh, transform.to_matrix(), inverse)))
        })
        .collect()
}

/// The nearest surface of `placed` straight below `from`, and the index of its model.
fn first_below(placed: &[(usize, Placed)], from: Vec3) -> Option<(usize, PlacedHit)> {
    let ray = Ray::new(from, -Vec3::Z);
    placed
        .iter()
        .filter_map(|(id, (mesh, bvh, matrix, inverse))| {
            let hit = raycast_placed_within(mesh, bvh, *matrix, *inverse, &ray, Scalar::INFINITY)?;
            Some((*id, hit))
        })
        .min_by(|(_, one), (_, other)| one.distance.total_cmp(&other.distance))
}

/// Where each island of `risks` stands a support: on the plate under it, at the top of the
/// layer below, mm. The stack was measured on `panel` and cut by `plan`.
pub fn island_floors(risks: &[Risk], plan: &LayerPlan, panel: &RasterSettings) -> Vec<Vec3> {
    risks
        .iter()
        .filter(|risk| {
            matches!(
                risk.kind,
                RiskKind::Island { .. } | RiskKind::Floating { .. }
            )
        })
        .filter_map(|risk| {
            let floor_mm = plan.top_of(risk.layer.checked_sub(1)?)?;
            Some(panel.plate_mm(risk.at_mm).extend(floor_mm))
        })
        .collect()
}

/// How far under a face a ray starts so as not to meet that face again, mm.
const THROUGH_FACE_MM: Scalar = 1e-3;
/// How far past a cup's floor a vent's tip reaches, mm: enough to be open whatever the floor's
/// facets do under its mouth.
const VENT_PAST_FLOOR_MM: Scalar = 1.0;

/// A model an island may hang from: its mesh, that mesh's bvh and where it is placed.
pub type Hanging<'a> = (&'a Mesh, &'a Bvh, Transform);

/// For each of `floors`, the model of `models` its island hangs from and the point of that
/// model's underside a support there touches: the first surface straight over a point a layer
/// under the floor, when it is no more than two layers up; `None` for an island of none.
pub fn island_holds(
    floors: &[Vec3],
    layer_mm: Scalar,
    models: &[Hanging],
) -> Vec<Option<(usize, Vec3)>> {
    let placed = placed(models);
    floors
        .iter()
        .map(|floor| {
            let ray = Ray::new(*floor - Vec3::Z * layer_mm, Vec3::Z);
            let within = UNDERSIDE_LAYERS * layer_mm;
            placed
                .iter()
                .filter_map(|(id, (mesh, bvh, matrix, inverse))| {
                    let hit = raycast_placed_within(mesh, bvh, *matrix, *inverse, &ray, within)?;
                    Some((*id, hit))
                })
                .min_by(|(_, one), (_, other)| one.distance.total_cmp(&other.distance))
                .map(|(id, hit)| (id, hit.point))
        })
        .collect()
}

/// A way for the air into a suction cup: a hole up through the floor that seals it, in plate
/// millimetres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CupVent {
    /// Index into the models given to [`cup_vents`].
    pub model: usize,
    pub mouth_mm: Vec3,
    /// Outward, of the surface the mouth is on.
    pub normal: Vec3,
    pub depth_mm: Scalar,
}

/// For each suction cup of `risks`, the hole through its floor; `None` for one whose floor
/// is the plate, or whose middle stands over no floor.
pub fn cup_vents(
    risks: &[Risk],
    plan: &LayerPlan,
    panel: &RasterSettings,
    models: &[Hanging],
) -> Vec<Option<CupVent>> {
    let placed = placed(models);
    let layer_mm = plan.nominal_thickness();
    let cups = risks
        .iter()
        .filter(|risk| matches!(risk.kind, RiskKind::SuctionCup { .. }));
    cups.map(|risk| {
        let in_cup = panel.plate_mm(risk.at_mm).extend(plan.top_of(risk.layer)?);
        let (model, floor) = first_below(&placed, in_cup).filter(|(_, hit)| hit.normal.z > 0.0)?;
        let under_floor = floor.point - Vec3::Z * THROUGH_FACE_MM;
        let own = placed.iter().position(|(id, _)| *id == model)?;
        let (_, exit) = first_below(&placed[own..=own], under_floor)?;
        (exit.normal.z < 0.0 && exit.point.z >= layer_mm).then_some(CupVent {
            model,
            mouth_mm: exit.point,
            normal: exit.normal,
            depth_mm: floor.point.z - exit.point.z + VENT_PAST_FLOOR_MM,
        })
    })
    .collect()
}

/// For each weak neck of `risks`, its model and the underside straight below the piece that
/// pulls on it; `None` for a piece over no underside, or whose underside lies on the plate.
pub fn lever_holds(
    risks: &[Risk],
    plan: &LayerPlan,
    panel: &RasterSettings,
    models: &[Hanging],
) -> Vec<Option<(usize, Vec3)>> {
    let placed = placed(models);
    let layer_mm = plan.nominal_thickness();
    let levers = risks
        .iter()
        .filter(|risk| matches!(risk.kind, RiskKind::Lever { .. }));
    levers
        .map(|risk| {
            let floor_mm = plan.top_of(risk.layer.checked_sub(1)?)?;
            let in_piece = panel.plate_mm(risk.at_mm).extend(floor_mm + layer_mm / 2.0);
            let (model, under) = first_below(&placed, in_piece)?;
            (under.normal.z < 0.0 && under.point.z >= layer_mm).then_some((model, under.point))
        })
        .collect()
}

#[cfg(test)]
mod tests {
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
        let (model, at) =
            lever_holds(&risks, &plan, &panel, &models)[0].expect("an underside at 4 mm");
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
        assert_eq!(lever_holds(&risks, &plan, &panel, &models), vec![None]);
    }
}
