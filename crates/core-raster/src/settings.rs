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

/// The greys an edge pixel is written in, as the vendor's slicer calls its Gray Range: an
/// edge grey is first raised to `min`, then the whole span squeezed into `min..=max`.
/// Black and full white are left alone; see `docs/decisions/0379`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GreyRange {
    pub min: u8,
    pub max: u8,
}

impl GreyRange {
    /// Every grey as it is.
    pub const FULL: Self = Self { min: 0, max: 255 };

    /// The grey `value` is written as.
    pub fn spread(self, value: u8) -> u8 {
        if value == 0 || value == u8::MAX {
            return value;
        }
        let lifted = f32::from(value.max(self.min));
        let span = f32::from(self.max) - f32::from(self.min);
        (f32::from(self.min) + lifted * span / 255.0).round() as u8
    }
}

impl Default for GreyRange {
    fn default() -> Self {
        Self::FULL
    }
}

/// How the greys of an edge are written: the span they are spread over and the rungs they
/// are rounded to; see `docs/design/rasterisation.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Grey {
    pub range: GreyRange,
    /// How many greys to round to, white included, or `None` to keep all 255.
    pub levels: Option<NonZeroU8>,
}

impl Grey {
    /// The 8-bit grey a coverage of `0.0..=1.0` is written as, before the range. Eight levels are
    /// `31, 63, ..., 255`; a count that does not divide 256 lands a little low.
    fn ladder(self, coverage: f32) -> u8 {
        let Some(levels) = self.levels else {
            return (coverage * 255.0).round() as u8;
        };
        let levels = u32::from(levels.get());
        let step = 256 / levels;
        let rung = ((coverage * levels as f32).ceil() as u32).clamp(1, levels);
        (rung * step - 1).min(255) as u8
    }

    /// The grey to write for a coverage.
    pub fn shade(self, coverage: f32) -> u8 {
        self.range.spread(self.ladder(coverage))
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

/// The anti-alias level a file states while the masks carry no grey: the vendor slicer's
/// default choice, inert while its flag is off; see `docs/decisions/0365`.
pub const DORMANT_ANTI_ALIAS_LEVEL: u16 = 4;

/// The edge blur a file states while there is none, by the same rule.
pub const DORMANT_BLUR_PX: u8 = 2;

impl RasterSettings {
    /// The anti-alias level a sliced file states: how many greys an edge is drawn in, at
    /// most `cap`, the container's own limit, and the dormant default for a binary panel.
    pub fn anti_alias_level(&self, cap: u16) -> u16 {
        match (self.shading, self.grey.levels) {
            (Shading::Binary, _) => DORMANT_ANTI_ALIAS_LEVEL.min(cap),
            (Shading::Coverage, Some(levels)) => u16::from(levels.get()).min(cap),
            (Shading::Coverage, None) => cap,
        }
    }

    /// The edge blur a sliced file states, the dormant default where there is none.
    pub fn stated_blur_px(&self) -> u8 {
        if self.blur_px == 0 {
            DORMANT_BLUR_PX
        } else {
            self.blur_px
        }
    }

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
        let GreyRange { min, max } = self.grey.range;
        if min > max {
            return Err(RasterError::InvertedGreyRange { min, max });
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
    fn the_anti_alias_level_is_how_many_greys_an_edge_is_drawn_in() {
        let with = |shading, levels| RasterSettings {
            shading,
            grey: Grey {
                levels: NonZeroU8::new(levels),
                ..Grey::default()
            },
            ..settings()
        };
        assert_eq!(
            with(Shading::Binary, 8).anti_alias_level(16),
            4,
            "the vendor's dormant default"
        );
        assert_eq!(with(Shading::Coverage, 4).anti_alias_level(16), 4);
        assert_eq!(with(Shading::Coverage, 64).anti_alias_level(16), 16);
        assert_eq!(
            with(Shading::Coverage, 0).anti_alias_level(16),
            16,
            "every grey"
        );
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
            levels: NonZeroU8::new(8),
            ..Grey::default()
        };
        // 31, 63, ..., 255: one rung per eighth of a pixel covered.
        let rungs: Vec<u8> = (1..=8).map(|n| grey.shade(n as f32 / 8.0)).collect();
        assert_eq!(rungs, vec![31, 63, 95, 127, 159, 191, 223, 255]);
    }

    #[test]
    fn an_edge_grey_is_spread_over_the_range_as_the_vendor_writes_it() {
        // Read off the vendor's files for one model at 0..255, 127..255 and 0..200.
        let lifted = GreyRange { min: 127, max: 255 };
        assert_eq!(lifted.spread(3), 191, "raised to 127, then spread");
        assert_eq!(lifted.spread(127), 191);
        assert_eq!(lifted.spread(144), 199);
        assert_eq!(lifted.spread(240), 247);
        let capped = GreyRange { min: 0, max: 200 };
        assert_eq!(capped.spread(64), 50);
        assert_eq!(capped.spread(248), 195);
        for range in [lifted, capped] {
            assert_eq!(range.spread(0), 0, "black stays black");
            assert_eq!(range.spread(255), 255, "full white stays white");
        }
        assert!((0..=255).all(|value| GreyRange::FULL.spread(value) == value));
    }

    #[test]
    fn a_range_whose_bottom_is_over_its_top_is_rejected() {
        let inverted = RasterSettings {
            grey: Grey {
                range: GreyRange { min: 200, max: 100 },
                levels: None,
            },
            ..settings()
        };
        assert!(matches!(
            inverted.validate(),
            Err(RasterError::InvertedGreyRange { min: 200, max: 100 })
        ));
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
