use core_geometry::Scalar;
use i_overlay::core::fill_rule::FillRule;
use i_overlay::float::simplify::SimplifyShape;

use crate::Contour;

/// The material one plane's contours cover, square millimetres, by the positive winding
/// rule the rasteriser fills by: what two overlapping bodies share is counted once, and a
/// hole cut through both takes its area off once.
///
/// The whole plane goes in at once, because which contour overlaps which is what the
/// figure is about. See `docs/decisions/0206`.
pub fn covered_area(contours: &[Contour]) -> Scalar {
    if contours.is_empty() {
        return 0.0;
    }
    let shape: Vec<Vec<[Scalar; 2]>> = contours
        .iter()
        .map(|contour| contour.points.iter().map(|p| [p.x, p.y]).collect())
        .collect();

    // Positive, the rule `core-raster` fills by: material is where the winding is above
    // zero, so eight copies of one cut leave one hole and not a solid wound minus seven.
    shape
        .simplify_shape(FillRule::Positive)
        .iter()
        .flatten()
        .map(|ring| shoelace(ring))
        .sum::<Scalar>()
        .abs()
        / 2.0
}

/// Twice the signed area of a ring, in the order the overlay handed it back.
fn shoelace(ring: &[[Scalar; 2]]) -> Scalar {
    let count = ring.len();
    (0..count)
        .map(|index| {
            let [px, py] = ring[index];
            let [qx, qy] = ring[(index + 1) % count];
            px.mul_add(qy, -(qx * py))
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Winding;
    use core_geometry::Vec2;

    /// A square of `side`, its lower left corner at `at`, wound counter-clockwise.
    fn square(at: Vec2, side: Scalar) -> Contour {
        Contour::new(
            vec![
                at,
                at + Vec2::new(side, 0.0),
                at + Vec2::new(side, side),
                at + Vec2::new(0.0, side),
            ],
            Winding::Outer,
        )
    }

    /// The same square wound clockwise, which is how a hole arrives.
    fn hole(at: Vec2, side: Scalar) -> Contour {
        let mut points = square(at, side).points;
        points.reverse();
        Contour::new(points, Winding::Inner)
    }

    #[test]
    fn one_square_covers_its_own_area() {
        let area = covered_area(&[square(Vec2::ZERO, 2.0)]);
        assert!((area - 4.0).abs() < 1e-3, "2 x 2 mm is 4 mm^2: {area}");
    }

    #[test]
    fn two_squares_overlapping_by_a_quarter_are_counted_once() {
        let area = covered_area(&[square(Vec2::ZERO, 2.0), square(Vec2::splat(1.0), 2.0)]);
        // Two 4 mm^2 squares sharing a 1 x 1 mm corner: 4 + 4 - 1.
        assert!((area - 7.0).abs() < 1e-3, "{area}");
    }

    #[test]
    fn a_hole_subtracts_itself_once_however_often_it_is_cut() {
        let laid_once = covered_area(&[square(Vec2::ZERO, 4.0), hole(Vec2::splat(1.0), 2.0)]);
        assert!((laid_once - 12.0).abs() < 1e-3, "16 - 4: {laid_once}");

        let mut eight = vec![square(Vec2::ZERO, 4.0)];
        eight.extend((0..8).map(|_| hole(Vec2::splat(1.0), 2.0)));
        let laid_eight = covered_area(&eight);
        assert!(
            (laid_eight - 12.0).abs() < 1e-3,
            "the same cut eight times over is still one hole: {laid_eight}"
        );
    }

    #[test]
    fn no_contours_cover_nothing() {
        assert!(covered_area(&[]) < Scalar::EPSILON);
    }
}
