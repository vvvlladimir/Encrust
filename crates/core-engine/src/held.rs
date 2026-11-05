//! The islands a measured stack shows, held up: where each stands a support and the surface
//! that support touches, as Check's Support every island stands one under each.

use core_analysis::{Risk, RiskKind};
use core_geometry::{Bvh, Mesh, Ray, Scalar, Transform, Vec3, raycast_placed_within};
use core_raster::RasterSettings;
use core_slicer::LayerPlan;

/// How many layers over an island's floor its underside may first be found.
const UNDERSIDE_LAYERS: Scalar = 2.0;

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
    let placed: Vec<_> = models
        .iter()
        .filter_map(|(mesh, bvh, transform)| {
            let inverse = transform.inverse_matrix()?;
            Some((*mesh, *bvh, transform.to_matrix(), inverse))
        })
        .enumerate()
        .collect();
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
}
