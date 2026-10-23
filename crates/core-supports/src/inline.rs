use core_geometry::{Scalar, Vec3};

use crate::edit::on_model;
use crate::placed::Placed;
use crate::region::WallSide;

/// How far apart a line of supports stands unless asked otherwise, millimetres.
pub const INLINE_SPACING_MM: Scalar = 4.0;

/// A spacing under this would line a stroke with more supports than resin; it is a tenth
/// of the narrowest head any profile ships.
const MIN_SPACING_MM: Scalar = 0.05;

/// Two points of a line closer than this share of its spacing are one support: the
/// surface nearest a chord bunches its samples where it folds away from it.
const CROWD: Scalar = 0.5;

/// Contacts in a line from the support at `from` to `to`, evenly spaced no further apart
/// than `spacing_mm`, each on the model's surface. In plate coordinates.
///
/// The line is laid along the chord between the two and every point taken to the nearest
/// surface, so it follows the model's underside without a path over its faces. `from`
/// stands already and is left out, `to` is the last point. A point that comes down on a
/// blocked face or crowds the one before it is left out too.
///
/// On a hollow model the surface is that of `side`, so a chord dipping into a thin wall
/// is not taken through it onto the cavity.
pub fn inline(
    placed: &Placed,
    from: Vec3,
    to: Vec3,
    spacing_mm: Scalar,
    side: Option<&WallSide>,
) -> Vec<Vec3> {
    if spacing_mm.is_nan() || spacing_mm < MIN_SPACING_MM {
        return Vec::new();
    }
    let steps = ((to - from).length() / spacing_mm).ceil().max(1.0) as usize;
    let mut last = from;
    let mut points = Vec::with_capacity(steps);
    for step in 1..=steps {
        let at = from.lerp(to, step as Scalar / steps as Scalar);
        let point = match side {
            Some(side) => on_side(placed, side, at),
            None => on_model(placed, at),
        };
        let Some(point) = point else {
            continue;
        };
        if point.distance(last) >= spacing_mm * CROWD {
            points.push(point);
            last = point;
        }
    }
    points
}

/// [`on_model`], kept to the faces on `side` of the wall.
fn on_side(placed: &Placed, side: &WallSide, at: Vec3) -> Option<Vec3> {
    let local = side.into_model.transform_point3(at);
    let kept = |face: usize| side.inner.contains(&face) == side.inside;
    let closest = side.bvh.closest_where(local, kept)?.point;
    let closest = side.into_model.inverse().transform_point3(closest);
    (!placed.blocks_at(closest)).then_some(closest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Blocked;
    use crate::tests::box_mesh;
    use core_geometry::{Bvh, Region, Transform};

    /// A 10 mm box floating 5 mm over the plate.
    fn floating() -> core_geometry::Mesh {
        box_mesh(Vec3::new(-5.0, -5.0, 5.0), Vec3::new(5.0, 5.0, 15.0))
    }

    #[test]
    fn a_line_under_a_face_is_evenly_spaced_and_ends_on_the_second_point() {
        let model = floating();
        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());
        let from = Vec3::new(-4.0, 0.0, 5.0);
        let to = Vec3::new(4.0, 0.0, 5.0);

        let points = inline(&placed, from, to, 3.0, None);

        // 8 mm at no more than 3 mm apart is three steps of 8/3 mm.
        let expected = [-4.0 + 8.0 / 3.0, -4.0 + 16.0 / 3.0, 4.0];
        assert_eq!(points.len(), expected.len(), "8 mm at 3 mm is three steps");
        for (point, x) in points.iter().zip(expected) {
            assert!(
                point.distance(Vec3::new(x, 0.0, 5.0)) < 1e-4,
                "{point} should lie on the underside at x = {x}"
            );
        }
    }

    #[test]
    fn a_chord_through_the_solid_is_brought_onto_its_surface() {
        let model = floating();
        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());
        // From the underside round the corner onto the side wall at x = 5.
        let points = inline(
            &placed,
            Vec3::new(0.0, 0.0, 5.0),
            Vec3::new(5.0, 0.0, 10.0),
            1.0,
            None,
        );

        assert!(!points.is_empty());
        for point in &points {
            let on_floor = (point.z - 5.0).abs() < 1e-4;
            let on_wall = (point.x - 5.0).abs() < 1e-4;
            assert!(on_floor || on_wall, "{point} is off the box's surface");
        }
    }

    #[test]
    fn a_line_over_a_blocked_face_leaves_it_bare() {
        let model = floating();
        let bvh = Bvh::build(&model);
        let mut region = Region::default();
        // The box's two underside triangles.
        region.set(0, true);
        region.set(1, true);
        let blocked =
            Blocked::new(&model, &region, Transform::default()).expect("two faces are marked");
        let placed = Placed::new(&model, &bvh, Transform::default()).blocking(Some(&blocked));

        let points = inline(
            &placed,
            Vec3::new(-4.0, 0.0, 5.0),
            Vec3::new(4.0, 0.0, 5.0),
            2.0,
            None,
        );

        assert!(points.is_empty(), "every point fell on a blocked face");
    }

    #[test]
    fn a_spacing_too_fine_to_print_lays_nothing() {
        let model = floating();
        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());
        let points = inline(
            &placed,
            Vec3::new(-4.0, 0.0, 5.0),
            Vec3::new(4.0, 0.0, 5.0),
            0.0,
            None,
        );
        assert!(points.is_empty());
    }

    #[test]
    fn a_chord_through_a_thin_wall_keeps_to_its_side_of_it() {
        let outer = floating();
        let cavity = box_mesh(Vec3::new(-4.5, -4.5, 5.5), Vec3::new(4.5, 4.5, 14.5));
        let offset = outer.vertices.len() as u32;
        let vertices = [outer.vertices.clone(), cavity.vertices].concat();
        let faces: Vec<[u32; 3]> = outer
            .faces
            .iter()
            .copied()
            .chain(cavity.faces.iter().map(|face| face.map(|at| at + offset)))
            .collect();
        let inner = outer.faces.len()..faces.len();
        let shell = core_geometry::Mesh::new(vertices, faces);
        let bvh = std::sync::Arc::new(Bvh::build(&shell));
        let placed = Placed::new(&shell, &bvh, Transform::default());
        let side = WallSide {
            bvh: std::sync::Arc::clone(&bvh),
            inner,
            into_model: core_geometry::Mat4::IDENTITY,
            inside: false,
        };
        let (from, to) = (Vec3::new(0.0, 0.0, 5.0), Vec3::new(5.0, 0.0, 10.0));

        let through = inline(&placed, from, to, 1.0, None);
        assert!(
            through.iter().any(|point| (point.z - 5.5).abs() < 1e-4),
            "the chord comes nearer the cavity's floor, 0.5 mm in, than the underside"
        );
        let points = inline(&placed, from, to, 1.0, Some(&side));
        assert!(!points.is_empty());
        for point in &points {
            let on_floor = (point.z - 5.0).abs() < 1e-4;
            let on_wall = (point.x - 5.0).abs() < 1e-4;
            assert!(on_floor || on_wall, "{point} is off the box's outside");
        }
    }
}
