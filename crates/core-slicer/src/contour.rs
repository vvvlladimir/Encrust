use std::cmp::Ordering;

use core_geometry::{Scalar, Vec2};

/// Which side of a closed contour is solid material.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Winding {
    /// Counter-clockwise: encloses material.
    Outer,
    /// Clockwise: encloses a hole.
    Inner,
}

/// A closed polygon on one slicing plane. The closing edge is implicit.
#[derive(Debug, Clone, PartialEq)]
pub struct Contour {
    pub points: Vec<Vec2>,
    pub winding: Winding,
}

impl Contour {
    pub fn new(points: Vec<Vec2>, winding: Winding) -> Self {
        Self { points, winding }
    }

    /// Builds a contour from points already in order, reading the winding off their
    /// signed area. Returns `None` when they enclose no area at all.
    pub fn from_points(points: Vec<Vec2>) -> Option<Self> {
        if points.len() < 3 {
            return None;
        }
        let winding = match signed_double_area(&points).partial_cmp(&0.0)? {
            Ordering::Greater => Winding::Outer,
            Ordering::Less => Winding::Inner,
            Ordering::Equal => return None,
        };
        Some(Self { points, winding })
    }

    /// Twice the signed area; positive when the points run counter-clockwise.
    pub fn signed_double_area(&self) -> Scalar {
        signed_double_area(&self.points)
    }

    /// Area enclosed by the contour, always positive, square millimetres.
    pub fn area(&self) -> Scalar {
        self.signed_double_area().abs() / 2.0
    }
}

/// The shoelace sum over a closed ring of points, positive for counter-clockwise order.
fn signed_double_area(points: &[Vec2]) -> Scalar {
    let n = points.len();
    (0..n)
        .map(|i| {
            let p = points[i];
            let q = points[(i + 1) % n];
            p.x.mul_add(q.y, -(q.x * p.y))
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_clockwise_unit_square_has_area_one() {
        let square = Contour::new(
            vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(1.0, 0.0),
                Vec2::new(1.0, 1.0),
                Vec2::new(0.0, 1.0),
            ],
            Winding::Outer,
        );
        assert!((square.signed_double_area() / 2.0 - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_clockwise_ring_is_read_as_a_hole() {
        let hole = Contour::from_points(vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(0.0, 1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(1.0, 0.0),
        ])
        .expect("a square encloses area");
        assert_eq!(hole.winding, Winding::Inner);
        assert!((hole.area() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_ring_with_no_area_is_rejected() {
        assert!(Contour::from_points(vec![Vec2::ZERO, Vec2::X]).is_none());
        assert!(
            Contour::from_points(vec![Vec2::ZERO, Vec2::X, Vec2::new(2.0, 0.0)]).is_none(),
            "three collinear points enclose nothing"
        );
    }

    #[test]
    fn reversing_the_points_flips_the_sign() {
        let mut points = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(2.0, 0.0),
            Vec2::new(2.0, 3.0),
        ];
        let ccw = Contour::new(points.clone(), Winding::Outer).signed_double_area();
        points.reverse();
        let cw = Contour::new(points, Winding::Inner).signed_double_area();
        assert!((ccw + cw).abs() < 1e-6);
    }
}
