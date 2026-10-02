use core_geometry::{Scalar, Vec2};
use i_overlay::mesh::float::outline::offset::OutlineOffset;
use i_overlay::mesh::float::style::OutlineStyle;

use crate::Contour;

/// Below this the walls have not moved and the contours are handed straight back:
/// a tenth of a micron is under any panel's pixel by three orders of magnitude.
const STILL_MM: Scalar = 1e-4;

/// Moves the walls of one plane's contours: `outer_mm` outwards on every outer contour
/// and `hole_mm` inwards on every hole, so a positive value of either leaves more
/// material.
///
/// The whole plane goes in at once, because which contour is a hole in which is what
/// decides where a wall ends up. Contours that close up under the offset are dropped, and
/// contours the offset runs together come out merged. See `docs/design/compensation.md`.
pub fn offset_contours(contours: &[Contour], hole_mm: Scalar, outer_mm: Scalar) -> Vec<Contour> {
    if contours.is_empty() || (hole_mm.abs() < STILL_MM && outer_mm.abs() < STILL_MM) {
        return contours.to_vec();
    }
    let shape: Vec<Vec<[Scalar; 2]>> = contours
        .iter()
        .map(|contour| contour.points.iter().map(|p| [p.x, p.y]).collect())
        .collect();

    let style = OutlineStyle::new(outer_mm)
        .outer_offset(outer_mm)
        .inner_offset(hole_mm);

    shape
        .outline(&style)
        .into_iter()
        .flatten()
        .filter_map(|ring| {
            let points: Vec<Vec2> = ring.into_iter().map(|[x, y]| Vec2::new(x, y)).collect();
            Contour::from_points(points)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Winding;

    /// A square of `side`, counter-clockwise from the origin.
    fn square(side: Scalar) -> Contour {
        Contour::new(
            vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(side, 0.0),
                Vec2::new(side, side),
                Vec2::new(0.0, side),
            ],
            Winding::Outer,
        )
    }

    /// A clockwise square of `side`, its lower left corner at `at`.
    fn hole(at: Scalar, side: Scalar) -> Contour {
        Contour::new(
            vec![
                Vec2::new(at, at),
                Vec2::new(at, at + side),
                Vec2::new(at + side, at + side),
                Vec2::new(at + side, at),
            ],
            Winding::Inner,
        )
    }

    #[test]
    fn no_offset_hands_the_contours_straight_back() {
        let plate = vec![square(10.0)];
        assert_eq!(offset_contours(&plate, 0.0, 0.0), plate);
    }

    #[test]
    fn an_outer_wall_moves_out_by_the_offset() {
        let grown = offset_contours(&[square(10.0)], 0.0, 0.1);
        assert_eq!(grown.len(), 1, "one square stays one contour");
        // A bevel join squares the corners off, so the area is the band plus two corners.
        let expected = 10.0 * 10.0 + 4.0 * 10.0 * 0.1 + 2.0 * 0.1 * 0.1;
        assert!(
            (grown[0].area() - expected).abs() < 1e-3,
            "got {}, expected {expected}",
            grown[0].area()
        );
        assert_eq!(grown[0].winding, Winding::Outer);
    }

    #[test]
    fn an_outer_wall_moves_in_on_a_negative_offset() {
        let shrunk = offset_contours(&[square(10.0)], 0.0, -0.5);
        assert!(
            (shrunk[0].area() - 9.0 * 9.0).abs() < 1e-3,
            "half a millimetre off each side of a 10 mm square"
        );
    }

    #[test]
    fn a_hole_closes_up_by_twice_the_offset_and_keeps_its_winding() {
        let plate = vec![square(10.0), hole(4.0, 2.0)];
        let offset = offset_contours(&plate, 0.25, 0.0);

        let inner: Vec<_> = offset
            .iter()
            .filter(|contour| contour.winding == Winding::Inner)
            .collect();
        assert_eq!(inner.len(), 1, "the hole is still a hole");
        assert!(
            (inner[0].area() - 1.5 * 1.5).abs() < 1e-3,
            "a 2 mm hole closes to 1.5 mm, got {}",
            inner[0].area()
        );
    }

    #[test]
    fn the_two_offsets_are_independent() {
        let plate = vec![square(10.0), hole(4.0, 2.0)];
        let offset = offset_contours(&plate, 0.25, -0.5);

        let outer = offset
            .iter()
            .find(|contour| contour.winding == Winding::Outer)
            .expect("the body is still there");
        let inner = offset
            .iter()
            .find(|contour| contour.winding == Winding::Inner)
            .expect("the hole is still there");
        assert!((outer.area() - 9.0 * 9.0).abs() < 1e-3);
        assert!((inner.area() - 1.5 * 1.5).abs() < 1e-3);
    }

    #[test]
    fn a_hole_smaller_than_the_offset_closes_altogether() {
        let plate = vec![square(10.0), hole(4.0, 0.4)];
        let offset = offset_contours(&plate, 0.3, 0.0);
        assert!(
            offset
                .iter()
                .all(|contour| contour.winding == Winding::Outer),
            "nothing is left of a 0.4 mm hole closed by 0.3 mm a side"
        );
    }

    #[test]
    fn a_wall_thinner_than_the_offset_disappears() {
        let offset = offset_contours(&[square(0.4)], 0.0, -0.3);
        assert!(offset.is_empty(), "0.4 mm eaten from both sides is nothing");
    }
}
