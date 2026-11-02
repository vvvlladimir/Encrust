use core_geometry::Vec3;

use crate::settings::Rgb;

/// A point of interest on the plate, drawn over the picture as a disc where the eye sees it
/// and as a ring where the model hides it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Marker {
    /// Plate millimetres.
    pub at: Vec3,
    pub colour: Rgb,
}

/// Where a marker landed in one picture.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarkerSpot {
    /// Pixels from the image's left edge to the marker's centre.
    pub x_px: f32,
    /// Pixels from the image's top edge to the marker's centre.
    pub y_px: f32,
    /// Whether something drawn stands between the eye and the point.
    pub hidden: bool,
}

/// What lines a marker, so that it reads on the background and on the model alike.
const OUTLINE: Rgb = [0xF4, 0xF4, 0xF4];

/// Width of the outline round a marker, pixels.
const OUTLINE_PX: f32 = 1.5;

/// How deep in front of the point a surface may lie and still count as the point's own,
/// pixels of depth: a point on a face tilted up to ~75° off the eye still reads as seen.
const SURFACE_SLACK_PX: f32 = 3.0;

/// A marker's radius in a picture of this size: 1/48 of its shorter side, four pixels at least.
pub(crate) fn radius_px(width_px: u32, height_px: u32) -> f32 {
    (width_px.min(height_px) as f32 / 48.0).max(4.0)
}

/// Whether a point at `screen` (pixels, then depth in mm) lies behind what `depths` holds there;
/// `scale` is pixels per millimetre.
pub(crate) fn hidden(
    depths: &[f32],
    (width_px, height_px): (u32, u32),
    screen: Vec3,
    scale: f32,
) -> bool {
    let (x, y) = (screen.x.floor(), screen.y.floor());
    if x < 0.0 || y < 0.0 || x >= width_px as f32 || y >= height_px as f32 {
        return false;
    }
    let nearest = depths[(y as u32 * width_px + x as u32) as usize];
    nearest < screen.z - SURFACE_SLACK_PX / scale
}

/// Draws one marker centred on `spot`, over whatever is there.
pub(crate) fn draw(
    colours: &mut [Rgb],
    (width_px, height_px): (u32, u32),
    spot: MarkerSpot,
    colour: Rgb,
) {
    let radius = radius_px(width_px, height_px);
    let ring = (radius / 3.0).max(2.0);
    let outer = radius + OUTLINE_PX;
    let span = |centre: f32, limit: u32| {
        let low = (centre - outer).floor().max(0.0) as u32;
        let high = ((centre + outer).ceil().max(0.0) as u32).min(limit);
        low.min(limit)..high
    };
    for y in span(spot.y_px, height_px) {
        for x in span(spot.x_px, width_px) {
            let dx = x as f32 + 0.5 - spot.x_px;
            let dy = y as f32 + 0.5 - spot.y_px;
            let distance = (dx * dx + dy * dy).sqrt();
            let painted = if distance > outer {
                None
            } else if distance > radius {
                Some(OUTLINE)
            } else if !spot.hidden || distance > radius - ring {
                Some(colour)
            } else {
                None
            };
            if let Some(painted) = painted {
                colours[(y * width_px + x) as usize] = painted;
            }
        }
    }
}
