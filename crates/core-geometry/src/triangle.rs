use crate::{Scalar, Vec3};

/// A single triangle with its vertices in counter-clockwise order seen from outside.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Triangle {
    pub a: Vec3,
    pub b: Vec3,
    pub c: Vec3,
}

impl Triangle {
    pub fn new(a: Vec3, b: Vec3, c: Vec3) -> Self {
        Self { a, b, c }
    }

    /// Weights of `point` at `a`, `b` and `c`, or `None` for a triangle with no area.
    pub(crate) fn barycentric(&self, point: Vec3) -> Option<[Scalar; 3]> {
        let normal = self.normal_unnormalized();
        let twice_area = normal.length_squared();
        if twice_area <= Scalar::EPSILON {
            return None;
        }
        // A corner's weight is the area of the sub-triangle opposite it over the whole, and
        // `normal.dot` of a cross product is twice that area with its sign.
        let at_a = normal.dot((self.c - self.b).cross(point - self.b)) / twice_area;
        let at_b = normal.dot((self.a - self.c).cross(point - self.c)) / twice_area;
        Some([at_a, at_b, 1.0 - at_a - at_b])
    }

    /// Unnormalised normal; its length is twice the triangle area.
    pub fn normal_unnormalized(&self) -> Vec3 {
        (self.b - self.a).cross(self.c - self.a)
    }

    pub fn area(&self) -> Scalar {
        self.normal_unnormalized().length() / 2.0
    }

    /// Whether its corners enclose no area at all, which is what a face counts as broken for.
    pub fn is_degenerate(&self) -> bool {
        self.normal_unnormalized().length_squared() == 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_right_triangle_has_half_area_and_up_normal() {
        let t = Triangle::new(Vec3::ZERO, Vec3::X, Vec3::Y);
        assert!((t.area() - 0.5).abs() < Scalar::EPSILON);
        assert_eq!(t.normal_unnormalized().normalize(), Vec3::Z);
    }
}
