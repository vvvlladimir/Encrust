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

    /// Unnormalised normal; its length is twice the triangle area.
    pub fn normal_unnormalized(&self) -> Vec3 {
        (self.b - self.a).cross(self.c - self.a)
    }

    pub fn area(&self) -> Scalar {
        self.normal_unnormalized().length() / 2.0
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
