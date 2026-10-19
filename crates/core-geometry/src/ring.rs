//! A flat ring of points: its winding and what it encloses.

use crate::{Scalar, Vec2};

/// Twice the signed area a ring of points encloses, positive where it runs anticlockwise.
pub fn ring_area<I>(ring: I) -> Scalar
where
    I: IntoIterator<Item = Vec2>,
    I::IntoIter: Clone,
{
    let points = ring.into_iter();
    points
        .clone()
        .zip(points.cycle().skip(1))
        .map(|(a, b)| a.perp_dot(b))
        .sum()
}

/// Whether a ring of points encloses `point`, by the crossing number.
pub fn ring_contains<I>(ring: I, point: Vec2) -> bool
where
    I: IntoIterator<Item = Vec2>,
    I::IntoIter: Clone,
{
    let points = ring.into_iter();
    let mut inside = false;
    for (a, b) in points.clone().zip(points.cycle().skip(1)) {
        if (a.y > point.y) != (b.y > point.y) {
            let crossing = (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x;
            if point.x < crossing {
                inside = !inside;
            }
        }
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQUARE: [Vec2; 4] = [
        Vec2::new(0.0, 0.0),
        Vec2::new(2.0, 0.0),
        Vec2::new(2.0, 2.0),
        Vec2::new(0.0, 2.0),
    ];

    #[test]
    fn a_square_run_anticlockwise_has_twice_its_area_and_reversed_the_negative() {
        assert!(
            (ring_area(SQUARE) - 8.0).abs() < 1e-6,
            "a 2 mm square is 4 mm², twice 8"
        );
        assert!((ring_area(SQUARE.into_iter().rev()) + 8.0).abs() < 1e-6);
    }

    #[test]
    fn a_square_holds_its_middle_and_not_a_point_beside_it() {
        assert!(ring_contains(SQUARE, Vec2::ONE));
        assert!(!ring_contains(SQUARE, Vec2::new(3.0, 1.0)));
    }
}
