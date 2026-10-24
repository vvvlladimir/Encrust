use core_geometry::{Scalar, Vec2};
use i_overlay::core::fill_rule::FillRule;
use i_overlay::float::simplify::SimplifyShape;

use crate::Contour;

/// Area in mm2 the plane's contours cover by the positive winding rule: overlaps count once.
/// The whole plane goes in at once; see `docs/decisions/0206`.
pub fn covered_area(contours: &[Contour]) -> Scalar {
    covered_pieces(contours)
        .iter()
        .flatten()
        .map(|ring| shoelace(ring))
        .sum::<Scalar>()
        .abs()
        / 2.0
}

/// The separate pieces one plane's contours cover, each its outline and then its holes:
/// what a pull on the film is computed over, since two pieces pull apart.
pub fn covered_pieces(contours: &[Contour]) -> Vec<Vec<Vec<Vec2>>> {
    if contours.is_empty() {
        return Vec::new();
    }
    let shape: Vec<Vec<[Scalar; 2]>> = contours
        .iter()
        .map(|contour| contour.points.iter().map(|p| [p.x, p.y]).collect())
        .collect();

    // Positive, the rule `core-raster` fills by: material is where the winding is above
    // zero, so eight copies of one cut leave one hole and not a solid wound minus seven.
    shape
        .simplify_shape(FillRule::Positive)
        .into_iter()
        .map(|piece| {
            piece
                .into_iter()
                .map(|ring| ring.into_iter().map(|[x, y]| Vec2::new(x, y)).collect())
                .collect()
        })
        .collect()
}

/// Twice the signed area of a ring, in the order the overlay handed it back.
fn shoelace(ring: &[Vec2]) -> Scalar {
    let count = ring.len();
    (0..count)
        .map(|index| {
            let p = ring[index];
            let q = ring[(index + 1) % count];
            p.x.mul_add(q.y, -(q.x * p.y))
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Winding;

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
    fn two_squares_apart_are_two_pieces_and_a_hole_stays_with_its_own() {
        let pieces = covered_pieces(&[
            square(Vec2::ZERO, 4.0),
            hole(Vec2::splat(1.0), 2.0),
            square(Vec2::new(10.0, 0.0), 2.0),
        ]);
        let mut rings: Vec<usize> = pieces.iter().map(Vec::len).collect();
        rings.sort_unstable();
        assert_eq!(rings, [1, 2], "a plain square, and a square with its hole");
    }

    #[test]
    fn no_contours_cover_nothing() {
        assert!(covered_area(&[]) < Scalar::EPSILON);
    }
}
