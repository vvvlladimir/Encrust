use std::num::NonZeroU8;

use core_raster::{Grey, PixelPitch, RasterSettings, Shading};
use printer_profiles::PrinterProfile;

use crate::format::SlicedFormat;

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

/// The panel the masks are drawn for, straight off the printer profile. Masks go in as the plate
/// stands, near edge up; only the header carries the mirroring (ADR 0134).
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

/// The panel a file of `format` is laid out on: rows from the far edge of the plate, as the
/// vendor slicer lays them, unless an X-mirrored machine prints the plate as it stands (ADR 0366).
pub(crate) fn file_panel(
    settings: &RasterSettings,
    printer: &PrinterProfile,
    format: SlicedFormat,
) -> RasterSettings {
    RasterSettings {
        mirror_y: rows_from_far_edge(format.extension(), printer),
        ..*settings
    }
}

/// Whether a file of the container `extension` names, cut for `printer`, lays its rows from
/// the far edge of the plate (ADR 0366); a file read back is named by its reader the same way.
pub(crate) fn rows_from_far_edge(extension: &str, printer: &PrinterProfile) -> bool {
    match extension {
        "cxline" => false,
        "photons" => true,
        _ => !printer.mirror_x,
    }
}
