//! Filling triangles into a depth and a normal per pixel, shaded afterwards in one pass.

use core_geometry::Vec3;

/// One corner of a face: pixel coordinates with depth, and its normal in view coordinates.
pub(super) struct Corner {
    pub screen: Vec3,
    pub normal: Vec3,
}

/// The nearest surface under each pixel: its depth, infinite where none, and its normal,
/// not yet of unit length.
pub(super) struct Surfaces {
    pub width_px: u32,
    pub height_px: u32,
    pub depths: Vec<f32>,
    pub normals: Vec<Vec3>,
}

impl Surfaces {
    pub fn new(width_px: u32, height_px: u32) -> Self {
        let count = (width_px as usize) * (height_px as usize);
        Self {
            width_px,
            height_px,
            depths: vec![f32::INFINITY; count],
            normals: vec![Vec3::ZERO; count],
        }
    }

    /// The depth at a pixel, or `None` off the image or where no face is.
    pub fn depth(&self, x: i64, y: i64) -> Option<f32> {
        if x < 0 || y < 0 || x >= i64::from(self.width_px) || y >= i64::from(self.height_px) {
            return None;
        }
        let depth = self.depths[(y as u64 * u64::from(self.width_px) + x as u64) as usize];
        depth.is_finite().then_some(depth)
    }

    /// Fills one screen-space triangle, keeping whichever face is nearest the eye.
    pub fn fill(&mut self, corners: &[Corner; 3]) {
        let [a, b, c] = corners.each_ref().map(|corner| corner.screen);
        let area = edge(a, b, c.x, c.y);
        if area.abs() < f32::EPSILON {
            return;
        }
        let inverse = 1.0 / area;

        let (left, right) = bounds(a.x, b.x, c.x, self.width_px);
        let (top, bottom) = bounds(a.y, b.y, c.y, self.height_px);
        for y in top..bottom {
            for x in left..right {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                // Dividing by the signed area normalises both windings, so a mesh with
                // flipped faces fills the same pixels.
                let weights = [
                    edge(b, c, px, py) * inverse,
                    edge(c, a, px, py) * inverse,
                    edge(a, b, px, py) * inverse,
                ];
                if weights.iter().any(|weight| *weight < 0.0) {
                    continue;
                }

                let depth = weights[0] * a.z + weights[1] * b.z + weights[2] * c.z;
                let index = (y * self.width_px + x) as usize;
                if depth < self.depths[index] {
                    self.depths[index] = depth;
                    self.normals[index] = weights[0] * corners[0].normal
                        + weights[1] * corners[1].normal
                        + weights[2] * corners[2].normal;
                }
            }
        }
    }
}

fn edge(from: Vec3, to: Vec3, x: f32, y: f32) -> f32 {
    (to.x - from.x) * (y - from.y) - (to.y - from.y) * (x - from.x)
}

/// Half-open pixel range a triangle's extent covers, clamped to the image.
fn bounds(a: f32, b: f32, c: f32, limit_px: u32) -> (u32, u32) {
    let low = a.min(b).min(c).floor().max(0.0) as u32;
    let high = (a.max(b).max(c).ceil().max(0.0) as u32).min(limit_px);
    (low.min(limit_px), high)
}
