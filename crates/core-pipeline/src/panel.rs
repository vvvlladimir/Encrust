use std::num::NonZeroU8;

use core_raster::{Grey, GreyRange, PixelPitch, RasterSettings, Shading};
use printer_profiles::{Mirror, PrinterProfile};

use crate::format::SlicedFormat;

/// What a front end may set over the panel the printer profile describes.
#[derive(Debug, Default, Clone, Copy)]
pub struct PanelOverrides {
    pub shading: Shading,
    /// How many greys an anti-aliased edge is rounded to, or `None` for all 255.
    pub grey_levels: Option<NonZeroU8>,
    /// The greys an edge is spread over, or `None` for the printer profile's.
    pub grey_range: Option<GreyRange>,
    /// Radius an edge is faded over, pixels; `0` leaves it sharp.
    pub blur_px: u8,
}

/// The panel the masks are drawn for, straight off the printer profile, without grey on a
/// binary-only machine whatever was asked (ADR 0377). It sees the plate as it stands, near
/// edge up (ADR 0134); a written file lays it out by `file_layout`.
pub fn raster_settings(printer: &PrinterProfile, overrides: PanelOverrides) -> RasterSettings {
    let (x, y) = printer.display.pixel_pitch_mm();
    RasterSettings {
        width_px: printer.display.width_px,
        height_px: printer.display.height_px,
        pitch: PixelPitch { x, y },
        mirror_x: false,
        mirror_y: false,
        shading: if printer.display.binary_only {
            Shading::Binary
        } else {
            overrides.shading
        },
        grey: Grey {
            range: overrides.grey_range.unwrap_or(GreyRange {
                min: printer.display.grey_min,
                max: printer.display.grey_max,
            }),
            levels: overrides.grey_levels,
        },
        blur_px: overrides.blur_px,
    }
}

/// How a file lays the plate out: mirrored along X, and its rows from the far edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileLayout {
    pub mirror_x: bool,
    pub mirror_y: bool,
}

/// Machines whose maker's writer reverses the rows of an LCD mirror's mask instead of each
/// row, compared without case: their files hold the plate as it stands (ADR 0366).
const ROWS_INSTEAD_OF_X: [&str; 4] = [
    "elegoo saturn 3 ultra",
    "elegoo jupiter se",
    "elegoo saturn 4 ultra",
    "elegoo mars 5 ultra",
];

/// How a file of the container `extension` names lays the plate out for the machine called
/// `machine_name` whose panel takes it as `mirror` (ADR 0366); a file read back is named by its
/// reader the same way.
pub fn file_layout(extension: &str, mirror: Mirror, machine_name: &str) -> FileLayout {
    let by_rows = ROWS_INSTEAD_OF_X
        .iter()
        .any(|name| name.eq_ignore_ascii_case(machine_name.trim()));
    match extension {
        "cxline" => FileLayout {
            mirror_x: false,
            mirror_y: false,
        },
        "photons" => FileLayout {
            mirror_x: false,
            mirror_y: true,
        },
        _ if mirror.mirrors_x() && by_rows => FileLayout {
            mirror_x: false,
            mirror_y: false,
        },
        _ => FileLayout {
            mirror_x: mirror.mirrors_x(),
            mirror_y: true,
        },
    }
}

/// The panel a file of `format` for `printer` is drawn on.
pub(crate) fn file_panel(
    settings: &RasterSettings,
    printer: &PrinterProfile,
    format: SlicedFormat,
) -> RasterSettings {
    let layout = file_layout(format.extension(), printer.mirror, printer.machine_name());
    RasterSettings {
        mirror_x: layout.mirror_x,
        mirror_y: layout.mirror_y,
        ..*settings
    }
}
