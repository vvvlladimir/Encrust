//! Lighter levels of heavy meshes and which of them a frame draws, so a plate of many
//! large models keeps up on whatever card it is shown on; see `docs/design/viewport.md`.

mod allot;
mod budget;
mod chain;

use std::sync::Arc;

use core_geometry::{FastMap, Mesh};

use crate::camera::OrbitCamera;
use crate::scene::ObjectId;

pub use allot::{Candidate, Step, allot, refine};
pub use budget::Budget;
pub use chain::Lods;

/// What a frame draws in place of models' own meshes, by object, and whether those are
/// still changing from frame to frame: the shadow is baked only once they settle.
#[derive(Debug, Default)]
pub struct Light {
    pub meshes: FastMap<ObjectId, Arc<Mesh>>,
    pub settling: bool,
}

/// Pixels a millimetre spans `distance_mm` from the eye, in a view `height_px` tall.
pub fn px_per_mm(camera: &OrbitCamera, distance_mm: f32, height_px: f32) -> f32 {
    let at_mm = if camera.orthographic {
        camera.distance_mm
    } else {
        distance_mm
    };
    height_px / (2.0 * at_mm.max(1e-3) * (camera.fov_y_rad / 2.0).tan())
}

#[cfg(test)]
pub(crate) mod tests {
    use core_geometry::{Mesh, Scalar, Vec3};

    use crate::camera::OrbitCamera;

    /// A ball of radius `r` around the origin, `rings` by `segments` facets.
    pub(crate) fn ball(r: Scalar, rings: usize, segments: usize) -> Mesh {
        let at = |ring: usize, segment: usize| {
            let polar = std::f32::consts::PI * ring as Scalar / rings as Scalar;
            let around = std::f32::consts::TAU * segment as Scalar / segments as Scalar;
            Vec3::new(
                polar.sin() * around.cos(),
                polar.sin() * around.sin(),
                polar.cos(),
            ) * r
        };
        let mut vertices = vec![at(0, 0)];
        for ring in 1..rings {
            vertices.extend((0..segments).map(|segment| at(ring, segment)));
        }
        vertices.push(at(rings, 0));
        let index =
            |ring: usize, segment: usize| (1 + (ring - 1) * segments + segment % segments) as u32;
        let south = (vertices.len() - 1) as u32;
        let mut faces = Vec::new();
        for segment in 0..segments {
            faces.push([0, index(1, segment), index(1, segment + 1)]);
            faces.push([
                index(rings - 1, segment),
                south,
                index(rings - 1, segment + 1),
            ]);
        }
        for ring in 1..rings - 1 {
            for segment in 0..segments {
                faces.push([
                    index(ring, segment),
                    index(ring + 1, segment),
                    index(ring + 1, segment + 1),
                ]);
                faces.push([
                    index(ring, segment),
                    index(ring + 1, segment + 1),
                    index(ring, segment + 1),
                ]);
            }
        }
        Mesh::new(vertices, faces)
    }

    #[test]
    fn a_millimetre_spans_what_the_view_angle_gives_it() {
        let camera = OrbitCamera {
            fov_y_rad: std::f32::consts::FRAC_PI_2,
            ..OrbitCamera::default()
        };
        // A 90° view 500 mm away is 1000 mm tall, across 1000 pixels.
        let px = super::px_per_mm(&camera, 500.0, 1000.0);
        assert!((px - 1.0).abs() < 1e-4, "one pixel a millimetre, got {px}");
    }
}
