//! How the render view draws the plate as it will come off the printer, as the window asks
//! for it and as the shader reads it.

use core_geometry::Scalar;
use egui::Color32;

use crate::camera::OrbitCamera;
use crate::render::gpu::{Desk, Look};

/// How the render view draws the plate as it will come off the printer: in the resin's own
/// colour, its layers stepped in, with references of known size beside it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PrintLook {
    pub resin: Color32,
    pub layer_height_mm: Scalar,
    /// How clear the resin is, 0.0 opaque to 1.0 water-clear: `ResinDetails::clarity`.
    pub clarity: f32,
}

/// Millimetres of the clearest resin light crosses before it clouds over: a water-clear
/// part reads through at any size a printer makes.
const CLEAREST_MM: f32 = 300.0;
/// The same for a resin barely clear at all: a millimetre, so its edges only glow.
const CLOUDIEST_MM: f32 = 1.0;

impl PrintLook {
    /// The shader's look through `camera` at a view `aspect` wide to its height. An opaque
    /// resin carries no light, which is what turns the clear pass off.
    pub(super) fn shading(self, camera: &OrbitCamera, aspect: f32) -> Look {
        let clarity = self.clarity.clamp(0.0, 1.0);
        let attenuation_mm = match clarity > 0.0 {
            true => CLOUDIEST_MM * (CLEAREST_MM / CLOUDIEST_MM).powf(clarity),
            false => 0.0,
        };
        Look {
            layer_height_mm: self.layer_height_mm,
            attenuation_mm,
            pivot_mm: camera.distance_mm,
            projection: camera.projection(aspect),
            desk: Desk::default(),
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
    fn an_opaque_resin_carries_no_light_so_no_clear_pass_runs() {
        assert!(
            clear(0.0).attenuation_mm.abs() < f32::EPSILON,
            "expected 0.0"
        );
    }

    #[test]
    fn the_clearest_resin_carries_light_its_full_distance() {
        let carried = clear(1.0).attenuation_mm;
        assert!(
            (carried - CLEAREST_MM).abs() < 1e-2,
            "expected {CLEAREST_MM} mm, got {carried} mm"
        );
    }

    #[test]
    fn a_clearer_resin_carries_light_further() {
        let ladder: Vec<f32> = [0.1, 0.4, 0.7, 1.0]
            .map(|clarity| clear(clarity).attenuation_mm)
            .to_vec();
        assert!(
            ladder.windows(2).all(|pair| pair[0] < pair[1]),
            "got {ladder:?}"
        );
    }
}
