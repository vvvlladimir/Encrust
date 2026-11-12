//! How sharply the surface turns under a tip's mark: on an edge or a corner the scar breaks
//! the line the eye follows, where on a face it is a dot.

use core_geometry::{Bvh, Mesh, Scalar, Vec3, point_triangle};

/// Half the widest automatic tip, millimetres: a turn of the surface this close to a tip is
/// under its mark.
const SCAR_RADIUS_MM: Scalar = 0.2;
/// A surface turning past 30° under the mark, as a cosine: a rounding tighter than about
/// 0.4 mm radius, which the mark cuts into.
pub const EDGE_COS: Scalar = 0.866;

/// The least cosine between the surface's facing at `tip` and any face within its mark: 1 on
/// a flat face, 0 on a right-angled edge. `found` is scratch for the search.
pub fn turn_under(model: &Mesh, bvh: &Bvh, tip: Vec3, found: &mut Vec<u32>) -> Option<Scalar> {
    let closest = bvh.closest(tip)?;
    let facing = model
        .triangle(closest.face)?
        .normal_unnormalized()
        .normalize_or_zero();
    bvh.faces_within(closest.point, SCAR_RADIUS_MM, found);
    let least = found
        .iter()
        .filter_map(|&face| model.triangle(face as usize))
        .filter(|triangle| {
            point_triangle(closest.point, triangle).distance(closest.point) <= SCAR_RADIUS_MM
        })
        .map(|triangle| triangle.normal_unnormalized().normalize_or_zero())
        .filter(|normal| *normal != Vec3::ZERO)
        .map(|normal| normal.dot(facing))
        .fold(1.0, Scalar::min);
    Some(least)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turn_at(tip: Vec3) -> Scalar {
        let block = core_geometry::cuboid(Vec3::ZERO, Vec3::new(4.0, 4.0, 4.0));
        let bvh = Bvh::build(&block);
        turn_under(&block, &bvh, tip, &mut Vec::new()).expect("a closed cube has a surface")
    }

    #[test]
    fn a_tip_mid_face_sits_on_flat_surface() {
        let turn = turn_at(Vec3::new(2.0, 2.0, 0.0));
        assert!(
            (turn - 1.0).abs() < 1e-4,
            "the middle of a face is flat, got {turn}"
        );
    }

    #[test]
    fn a_tip_on_a_cube_edge_sits_on_a_right_angle() {
        let turn = turn_at(Vec3::new(2.0, 0.0, 0.0));
        assert!(
            turn.abs() < 1e-4,
            "a cube's edge turns 90°, cosine 0, got {turn}"
        );
        assert!(turn < EDGE_COS);
    }

    #[test]
    fn a_tip_clear_of_the_edge_by_more_than_its_mark_is_on_the_face() {
        let turn = turn_at(Vec3::new(2.0, 0.3, 0.0));
        assert!(
            (turn - 1.0).abs() < 1e-4,
            "0.3 mm off the edge is past a 0.2 mm mark, got {turn}"
        );
    }
}
