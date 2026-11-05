//! The islands a measured stack shows, held up: where each stands a support and the surface
//! that support touches, as Check's Support every island stands one under each.

use core_analysis::{Risk, RiskKind};
use core_geometry::{Bvh, Ray, Scalar, Vec3};
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

/// The surface `bvh` was built from straight over a point a layer under `floor`, and how far
/// up it is, when it is no more than two layers up: the underside of the island there.
pub fn island_underside(bvh: &Bvh, floor: Vec3, layer_mm: Scalar) -> Option<(Scalar, Vec3)> {
    let ray = Ray::new(floor - Vec3::Z * layer_mm, Vec3::Z);
    let hit = bvh.raycast_within(&ray, UNDERSIDE_LAYERS * layer_mm)?;
    Some((hit.t, ray.at(hit.t)))
}

#[cfg(test)]
mod tests {
    use core_geometry::cuboid;

    use super::*;

    #[test]
    fn an_island_a_layer_over_its_floor_is_found_on_the_underside_of_the_model() {
        let floating = cuboid(Vec3::new(0.0, 0.0, 5.03), Vec3::new(10.0, 10.0, 10.0));
        let bvh = Bvh::build(&floating);
        let (up, at) = island_underside(&bvh, Vec3::new(5.0, 5.0, 5.0), 0.05)
            .expect("the box's floor is inside the layer over the floor");
        assert!(
            (at.z - 5.03).abs() < 1e-4,
            "the box's floor at 5.03 mm, found {at}"
        );
        assert!(
            (up - 0.08).abs() < 1e-4,
            "0.08 mm up from a layer under, found {up}"
        );
    }

    #[test]
    fn a_surface_more_than_two_layers_up_holds_no_island_there() {
        let floating = cuboid(Vec3::new(0.0, 0.0, 6.0), Vec3::new(10.0, 10.0, 10.0));
        let bvh = Bvh::build(&floating);
        let found = island_underside(&bvh, Vec3::new(5.0, 5.0, 5.0), 0.05);
        assert_eq!(
            found, None,
            "1 mm up is another part's floor, not this island's"
        );
    }
}
