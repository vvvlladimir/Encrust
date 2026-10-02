use std::num::NonZeroU8;

use crate::RasterError;

/// Physical size of one LCD pixel, millimetres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PixelPitch {
    pub x: f32,
    pub y: f32,
}

/// How a pixel's exposure is decided from how much of it the layer covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Shading {
    /// Grey in proportion to the exact area of the pixel the layer covers. See
    /// `docs/design/rasterisation.md`.
    #[default]
    Coverage,
    /// Fully lit or fully dark, decided at the pixel's centre. For panels that do not
    /// honour intermediate grey.
    Binary,
}

/// How much of the grey range the panel and the resin between them can actually hold.
///
/// Coverage shading can ask for any of 255 greys, but a pixel dim enough leaves resin
/// uncured rather than partly cured. See `docs/design/rasterisation.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Grey {
    /// Dimmest grey the panel cures. Anything below it is written black instead.
    pub floor: u8,
    /// How many greys to round to, white included, or `None` to keep all 255.
    pub levels: Option<NonZeroU8>,
}

impl Grey {
    /// The 8-bit grey a coverage of `0.0..=1.0` is written as, before the floor.
    ///
    /// The ladder matches what other slicers write at the same level count: eight levels
    /// are `31, 63, ..., 255`. A count that does not divide 256 lands a little low.
    fn ladder(self, coverage: f32) -> u8 {
        let Some(levels) = self.levels else {
            return (coverage * 255.0).round() as u8;
        };
        let levels = u32::from(levels.get());
        let step = 256 / levels;
        let rung = ((coverage * levels as f32).ceil() as u32).clamp(1, levels);
        (rung * step - 1).min(255) as u8
    }

    /// The grey to write for a coverage, black where the panel would not cure it.
    pub fn shade(self, coverage: f32) -> u8 {
        let value = self.ladder(coverage);
        if value < self.floor { 0 } else { value }
    }
}

/// Everything the rasterizer needs about the machine it is drawing for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RasterSettings {
    pub width_px: u32,
    pub height_px: u32,
    pub pitch: PixelPitch,
    /// Whether the panel is mirrored relative to model space, from the printer profile.
    pub mirror_x: bool,
    pub mirror_y: bool,
    pub shading: Shading,
    /// What the panel does with a grey that is not black or white.
    pub grey: Grey,
    /// Radius of the box filter an edge is faded over, pixels; `0` leaves it sharp.
    pub blur_px: u8,
}

impl RasterSettings {
    /// Rejects a request that cannot produce a usable mask.
    pub fn validate(&self) -> Result<(), RasterError> {
        if self.width_px == 0 || self.height_px == 0 {
            return Err(RasterError::ZeroResolution {
                width: self.width_px,
                height: self.height_px,
            });
        }
        // Pixels are addressed by a `u32` index all the way to the file format, so a
        // panel whose pixel count does not fit in one cannot be described at all.
        if u64::from(self.width_px) * u64::from(self.height_px) > u64::from(u32::MAX) {
            return Err(RasterError::PanelTooLarge {
                width: self.width_px,
                height: self.height_px,
            });
        }
        if self.pitch.x <= 0.0 || self.pitch.y <= 0.0 {
            return Err(RasterError::NonPositivePixelPitch(
                self.pitch.x.min(self.pitch.y),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> RasterSettings {
        RasterSettings {
            width_px: 64,
            height_px: 32,
            pitch: PixelPitch { x: 0.1, y: 0.1 },
            mirror_x: false,
            mirror_y: false,
            shading: Shading::default(),
            grey: Grey::default(),
            blur_px: 0,
        }
    }

    #[test]
    fn full_range_grey_is_coverage_scaled_to_255() {
        let grey = Grey::default();
        assert_eq!(grey.shade(0.5), 128);
        assert_eq!(grey.shade(1.0), 255);
        assert_eq!(grey.shade(0.0), 0);
    }

    #[test]
    fn eight_levels_are_the_ladder_other_slicers_write() {
        let grey = Grey {
            floor: 0,
            levels: NonZeroU8::new(8),
        };
        // 31, 63, ..., 255: one rung per eighth of a pixel covered.
        let rungs: Vec<u8> = (1..=8).map(|n| grey.shade(n as f32 / 8.0)).collect();
        assert_eq!(rungs, vec![31, 63, 95, 127, 159, 191, 223, 255]);
    }

    #[test]
    fn a_grey_under_the_floor_is_written_black() {
        let grey = Grey {
            floor: 128,
            levels: None,
        };
        assert_eq!(
            grey.shade(0.1),
            0,
            "26 would not cure, so it is not written"
        );
        assert_eq!(grey.shade(0.5), 128, "the floor itself is kept");
        assert_eq!(grey.shade(1.0), 255, "white is never floored");
    }

    #[test]
    fn a_sound_request_validates() {
        assert!(settings().validate().is_ok());
    }

    #[test]
    fn zero_resolution_is_rejected() {
        let broken = RasterSettings {
            width_px: 0,
            ..settings()
        };
        assert!(matches!(
            broken.validate(),
            Err(RasterError::ZeroResolution { .. })
        ));
    }

    #[test]
    fn non_positive_pitch_is_rejected() {
        let broken = RasterSettings {
            pitch: PixelPitch { x: 0.0, y: 0.1 },
            ..settings()
        };
        assert!(matches!(
            broken.validate(),
            Err(RasterError::NonPositivePixelPitch(_))
        ));
    }

    #[test]
    fn a_panel_with_more_pixels_than_an_index_can_hold_is_rejected() {
        let broken = RasterSettings {
            width_px: 100_000,
            height_px: 100_000,
            ..settings()
        };
        assert!(matches!(
            broken.validate(),
            Err(RasterError::PanelTooLarge { .. })
        ));
    }
}
