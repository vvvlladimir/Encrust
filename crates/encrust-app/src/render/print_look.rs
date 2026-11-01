//! How the render view draws the plate as it will come off the printer, as the window asks
//! for it and as the shader reads it.

use core_geometry::{Aabb, Scalar};
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
    /// Whether what stands off the point the camera turns about is blurred, as by a lens.
    pub depth_of_field: bool,
}

/// The lens's aperture radius as a share of its distance to what it is focused on: a macro
/// shot of a miniature at f/8 or so, blurred enough to read, soft over sixteen frames.
const APERTURE_SHARE: f32 = 0.016;

/// How far the models' nearest and farthest points may blur, points: a pixel on a Retina
/// screen, which the frames' own jitter already softens by.
const MODEL_BLUR_PT: f32 = 0.5;

/// Millimetres of the clearest resin light crosses before it clouds over: a water-clear
/// part reads through at any size a printer makes.
const CLEAREST_MM: f32 = 300.0;
/// The same for a resin barely clear at all: a millimetre, so its edges only glow.
const CLOUDIEST_MM: f32 = 1.0;

/// Where the lens is focused and how wide it opens, mm.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Focus {
    distance_mm: f32,
    aperture_mm: f32,
}

impl PrintLook {
    /// The shader's look through `camera` at a view `size_pt` large, with the lens kept from
    /// blurring the `models`. An opaque resin carries no light, which turns the clear pass off.
    pub(super) fn shading(
        self,
        camera: &OrbitCamera,
        size_pt: egui::Vec2,
        models: Option<&Aabb>,
    ) -> Look {
        let clarity = self.clarity.clamp(0.0, 1.0);
        let attenuation_mm = match clarity > 0.0 {
            true => CLOUDIEST_MM * (CLEAREST_MM / CLOUDIEST_MM).powf(clarity),
            false => 0.0,
        };
        let focus = match self.depth_of_field && !camera.orthographic {
            true => focus_on(camera, size_pt.y, models),
            false => Focus {
                distance_mm: camera.distance_mm,
                aperture_mm: 0.0,
            },
        };
        Look {
            layer_height_mm: self.layer_height_mm,
            attenuation_mm,
            pivot_mm: camera.distance_mm,
            projection: camera.projection(size_pt.x / size_pt.y),
            focus_mm: focus.distance_mm,
            aperture_mm: focus.aperture_mm,
            desk: Desk::default(),
        }
    }
}

/// The lens through `camera` over a view `height_pt` tall: focused across the depth the
/// `models` span, open no wider than keeps both its ends within [`MODEL_BLUR_PT`].
fn focus_on(camera: &OrbitCamera, height_pt: f32, models: Option<&Aabb>) -> Focus {
    let eye = camera.eye();
    let forward = (camera.target - eye).normalize_or_zero();
    let (near_mm, far_mm) = models.map_or((camera.distance_mm, camera.distance_mm), |models| {
        let middle = (models.mins + models.maxs) / 2.0 - eye;
        let half = ((models.maxs - models.mins) / 2.0).dot(forward.abs());
        let centre = middle.dot(forward);
        ((centre - half).max(1e-3), (centre + half).max(1e-3))
    });
    // Equal blur at both ends puts the focus at the harmonic mean of their depths.
    let distance_mm = 2.0 * near_mm * far_mm / (near_mm + far_mm);
    let blur_pt_per_mm =
        (1.0 / near_mm - 1.0 / far_mm) / 2.0 * height_pt / (2.0 * (camera.fov_y_rad / 2.0).tan());
    let aperture_mm = match blur_pt_per_mm > 0.0 {
        true => (MODEL_BLUR_PT / blur_pt_per_mm).min(distance_mm * APERTURE_SHARE),
        false => distance_mm * APERTURE_SHARE,
    };
    Focus {
        distance_mm,
        aperture_mm,
    }
}

#[cfg(test)]
mod tests {
    use core_geometry::Vec3;

    use super::*;
    use crate::ui::theme;

    fn plain() -> PrintLook {
        PrintLook {
            resin: theme::resin_swatch([0x80, 0x80, 0x80]),
            layer_height_mm: 0.05,
            clarity: 0.0,
            depth_of_field: false,
        }
    }

    fn clear(clarity: f32) -> Look {
        PrintLook { clarity, ..plain() }.shading(
            &OrbitCamera::default(),
            egui::vec2(800.0, 800.0),
            None,
        )
    }

    /// Looking down −X from 100 mm at the middle of a cube 40 mm across.
    fn facing_a_cube() -> (OrbitCamera, Aabb) {
        let camera = OrbitCamera {
            yaw_rad: 0.0,
            pitch_rad: 0.0,
            distance_mm: 100.0,
            ..OrbitCamera::default()
        };
        let cube = Aabb::new(Vec3::splat(-20.0), Vec3::splat(20.0));
        (camera, cube)
    }

    #[test]
    fn the_lens_blurs_the_nearest_and_farthest_of_the_model_alike_and_no_further() {
        let (camera, cube) = facing_a_cube();
        let height_pt = 800.0;
        let focus = focus_on(&camera, height_pt, Some(&cube));
        // The cube spans 80 to 120 mm: their harmonic mean is (100² − 20²) / 100.
        assert!(
            (focus.distance_mm - 96.0).abs() < 1e-3,
            "expected 96 mm, got {} mm",
            focus.distance_mm
        );
        let blur_pt = |depth_mm: f32| {
            focus.aperture_mm * (1.0 / depth_mm - 1.0 / focus.distance_mm).abs() * height_pt
                / (2.0 * (camera.fov_y_rad / 2.0).tan())
        };
        for depth_mm in [80.0, 120.0] {
            assert!(
                (blur_pt(depth_mm) - MODEL_BLUR_PT).abs() < 1e-3,
                "expected {MODEL_BLUR_PT} pt at {depth_mm} mm, got {} pt",
                blur_pt(depth_mm)
            );
        }
    }

    #[test]
    fn with_no_model_the_lens_focuses_on_the_pivot_as_a_macro_lens() {
        let (camera, _) = facing_a_cube();
        let focus = focus_on(&camera, 800.0, None);
        let expected = Focus {
            distance_mm: 100.0,
            aperture_mm: 100.0 * APERTURE_SHARE,
        };
        assert_eq!(focus, expected);
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
