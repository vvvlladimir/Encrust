use std::num::NonZeroU8;

use core_raster::{Grey, PixelPitch, RasterSettings, Shading};
use printer_profiles::PrinterProfile;

/// What a front end may set over the panel the printer profile describes.
#[derive(Debug, Default, Clone, Copy)]
pub struct PanelOverrides {
    pub shading: Shading,
    /// How many greys an anti-aliased edge is rounded to, or `None` for all 255.
    pub grey_levels: Option<NonZeroU8>,
    /// Dimmest grey to write, or `None` to keep what the profile claims the panel cures.
    pub grey_floor: Option<u8>,
    /// Radius an edge is faded over, pixels; `0` leaves it sharp.
    pub blur_px: u8,
}

/// The panel the masks are drawn for, straight off the printer profile.
///
/// The masks go into the file the way the plate stands, from its near edge up; the
/// profile's mirroring is the panel's mounting and only the header carries it (ADR 0134).
pub fn raster_settings(printer: &PrinterProfile, overrides: PanelOverrides) -> RasterSettings {
    let (x, y) = printer.display.pixel_pitch_mm();
    RasterSettings {
        width_px: printer.display.width_px,
        height_px: printer.display.height_px,
        pitch: PixelPitch { x, y },
        mirror_x: false,
        mirror_y: false,
        shading: overrides.shading,
        grey: Grey {
            floor: overrides.grey_floor.unwrap_or(printer.display.grey_floor),
            levels: overrides.grey_levels,
        },
        blur_px: overrides.blur_px,
    }
}
