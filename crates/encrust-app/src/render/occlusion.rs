//! The render view's soft shading of creases, worked out from the depth plane as the scene is
//! copied out, and only while the camera rests. See docs/design/viewport.md.

use bytemuck::{Pod, Zeroable};
use core_geometry::Mat4;
use egui::epaint::ViewportInPixels;

/// Millimetres around a point that can shade it: the crevices of a miniature, not the room.
const REACH_MM: f32 = 4.0;
/// How dark a fully enclosed crease goes, as a share taken off its light.
const STRENGTH: f32 = 1.0;

/// The copy pass's uniform: how to turn a pixel and its depth back into a point the camera
/// sees, and how far and how hard to shade.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Occlusion {
    inverse_projection: [[f32; 4]; 4],
    projection: [[f32; 4]; 4],
    /// Left, top, width and height of the viewport, framebuffer pixels.
    viewport: [f32; 4],
    /// `x` 1.0 while creases are shaded, `y` the reach in mm, `z` the strength, `w` 1.0 in
    /// the render view, whose frame is vignetted as a lens darkens its corners.
    shading: [f32; 4],
}

impl Occlusion {
    /// Nothing shaded: the copy is the scene as drawn.
    pub fn off() -> Self {
        Self::zeroed()
    }

    /// The copy of a frame drawn into `viewport`: vignetted in the render view, `staged`, and
    /// its creases shaded through `projection` where there is one.
    pub fn of(viewport: &ViewportInPixels, staged: bool, projection: Option<Mat4>) -> Self {
        let vignette = if staged { 1.0 } else { 0.0 };
        let viewport = [
            viewport.left_px as f32,
            viewport.top_px as f32,
            viewport.width_px as f32,
            viewport.height_px as f32,
        ];
        let Some(projection) = projection else {
            return Self {
                viewport,
                shading: [0.0, 0.0, 0.0, vignette],
                ..Self::zeroed()
            };
        };
        Self {
            inverse_projection: projection.inverse().to_cols_array_2d(),
            projection: projection.to_cols_array_2d(),
            viewport,
            shading: [1.0, REACH_MM, STRENGTH, vignette],
        }
    }

    pub fn shades(&self) -> bool {
        self.shading[0] > 0.5
    }
}
