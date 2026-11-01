//! How the render view draws the plate as it will come off the printer, as the window asks
//! for it and as the shader reads it.

use core_geometry::Scalar;
use egui::Color32;

use crate::camera::OrbitCamera;
use crate::render::gpu::Look;

/// How the render view draws the plate as it will come off the printer: in the resin's own
/// colour, its layers stepped in, with references of known size beside it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PrintLook {
    pub resin: Color32,
    pub layer_height_mm: Scalar,
    /// How clear the resin is, 0.0 opaque to 1.0 water-clear: `ResinDetails::clarity`.
    pub clarity: f32,
}

/// What 10 mm of the clearest resin lets through: even a clear resin tints a thick part.
const CLEAREST_THROUGH_10_MM: f32 = 0.9;
/// The least a resin lets through 10 mm and still counts as clear, so absorption is finite.
const DARKEST_THROUGH_10_MM: f32 = 1e-3;

impl PrintLook {
    /// The shader's look through `camera` at a view `aspect` wide to its height. An opaque
    /// resin absorbs nothing, which is what turns the clear pass off.
    pub(super) fn shading(self, camera: &OrbitCamera, aspect: f32) -> Look {
        let clarity = self.clarity.clamp(0.0, 1.0);
        let through_10_mm = CLEAREST_THROUGH_10_MM * clarity * clarity;
        let absorption_per_mm = match clarity > 0.0 {
            true => -through_10_mm.max(DARKEST_THROUGH_10_MM).ln() / 10.0,
            false => 0.0,
        };
        Look {
            layer_height_mm: self.layer_height_mm,
            absorption_per_mm,
            pivot_mm: camera.distance_mm,
            projection: camera.projection(aspect),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme;

    fn clear(clarity: f32) -> Look {
        PrintLook {
            resin: theme::resin_swatch([0x80, 0x80, 0x80]),
            layer_height_mm: 0.05,
            clarity,
        }
        .shading(&OrbitCamera::default(), 1.0)
    }

    #[test]
    fn an_opaque_resin_absorbs_nothing_so_no_clear_pass_runs() {
        assert!(
            clear(0.0).absorption_per_mm.abs() < f32::EPSILON,
            "expected 0.0"
        );
    }

    #[test]
    fn the_clearest_resin_lets_its_share_through_10_mm() {
        let through = (-clear(1.0).absorption_per_mm * 10.0).exp();
        assert!(
            (through - CLEAREST_THROUGH_10_MM).abs() < 1e-4,
            "expected {CLEAREST_THROUGH_10_MM} through 10 mm, got {through}"
        );
    }

    #[test]
    fn a_clearer_resin_absorbs_less() {
        let ladder: Vec<f32> = [0.1, 0.4, 0.7, 1.0]
            .map(|clarity| clear(clarity).absorption_per_mm)
            .to_vec();
        assert!(
            ladder.windows(2).all(|pair| pair[0] > pair[1]),
            "got {ladder:?}"
        );
    }
}
