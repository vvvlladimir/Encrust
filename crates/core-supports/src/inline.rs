use core_geometry::{Scalar, Vec3};

use crate::edit::on_model;
use crate::placed::Placed;

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
pub fn inline(placed: &Placed, from: Vec3, to: Vec3, spacing_mm: Scalar) -> Vec<Vec3> {
    if spacing_mm.is_nan() || spacing_mm < MIN_SPACING_MM {
        return Vec::new();
    }
    let steps = ((to - from).length() / spacing_mm).ceil().max(1.0) as usize;
    let mut last = from;
    let mut points = Vec::with_capacity(steps);
    for step in 1..=steps {
        let Some(point) = on_model(placed, from.lerp(to, step as Scalar / steps as Scalar)) else {
            continue;
        };
        if point.distance(last) >= spacing_mm * CROWD {
            points.push(point);
            last = point;
        }
    }
    points
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

        let points = inline(&placed, from, to, 3.0);

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
        );
        assert!(points.is_empty());
    }
}
