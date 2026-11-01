//! The render view's soft shading of creases, worked out from the depth plane as the scene is
//! copied out, and only while the camera rests. See docs/design/viewport.md.

use bytemuck::{Pod, Zeroable};
use core_geometry::Mat4;
use egui::epaint::ViewportInPixels;

/// Millimetres around a point that can shade it: the crevices of a miniature, not the room.
const REACH_MM: f32 = 2.5;
/// How dark a fully enclosed crease goes, as a share taken off its light.
const STRENGTH: f32 = 1.1;

/// The copy pass's uniform: how to turn a pixel and its depth back into a point the camera
/// sees, and how far and how hard to shade.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Occlusion {
    inverse_projection: [[f32; 4]; 4],
    projection: [[f32; 4]; 4],
    /// Left, top, width and height of the viewport, framebuffer pixels.
    viewport: [f32; 4],
    /// `x` 1.0 while creases are shaded, `y` the reach in mm, `z` the strength.
    shading: [f32; 4],
}

impl Occlusion {
    /// Nothing shaded: the copy is the scene as drawn.
    pub fn off() -> Self {
        Self::zeroed()
    }

    /// Creases shaded for a camera seeing through `projection` into `viewport`.
    pub fn of(projection: Mat4, viewport: &ViewportInPixels) -> Self {
        Self {
            inverse_projection: projection.inverse().to_cols_array_2d(),
            projection: projection.to_cols_array_2d(),
            viewport: [
                viewport.left_px as f32,
                viewport.top_px as f32,
                viewport.width_px as f32,
                viewport.height_px as f32,
            ],
            shading: [1.0, REACH_MM, STRENGTH, 0.0],
        }
    }

    pub fn shades(&self) -> bool {
        self.shading[0] > 0.5
    }
}
