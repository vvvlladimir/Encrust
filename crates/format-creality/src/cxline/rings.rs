//! A layer as the container holds it: its area, then its rings as points in micrometres
//! from the panel's corner. See `docs/formats/cxline.md`.

use std::io::Cursor;

use core_format::{FormatError, Reads};
use core_raster::{Ring, area_mm2};
use core_slicer::Contour;
use glam::Vec2;

use crate::v3::blocks::PAGE_BREAK;

/// Bytes of one point: two signed words.
pub(crate) const POINT_BYTES: u64 = 8;

/// The area field of a layer: what it lights in square millimetres, times a thousand.
pub(crate) fn area_field(rings: &[Ring], pitch_mm: (f32, f32)) -> u32 {
    (area_mm2(rings, pitch_mm) * 1000.0)
        .round()
        .clamp(0.0, f64::from(u32::MAX)) as u32
}

/// A layer's bytes, page break included. Each ring is closed by its first point again.
pub(crate) fn layer_bytes(area: u32, rings: &[Ring], pitch_mm: (f32, f32)) -> Vec<u8> {
    let points: usize = rings.iter().map(|ring| ring.len() + 1).sum();
    let mut body = Vec::with_capacity(8 + 4 * rings.len() + 8 * points + PAGE_BREAK.len());
    body.extend_from_slice(&area.to_be_bytes());
    body.extend_from_slice(&(rings.len() as u32).to_be_bytes());
    let micrometres = |px: u32, pitch: f32| (px as f32 * pitch * 1000.0).round() as i32;
    for ring in rings {
        body.extend_from_slice(&(ring.len() as u32 + 1).to_be_bytes());
        for &(x, y) in ring.iter().chain(ring.first()) {
            body.extend_from_slice(&micrometres(x, pitch_mm.0).to_be_bytes());
            body.extend_from_slice(&micrometres(y, pitch_mm.1).to_be_bytes());
        }
    }
    body.extend_from_slice(&PAGE_BREAK);
    body
}

/// The rings of one layer's bytes, area and counts included, page break not, as contours in
/// millimetres from the panel's corner.
pub(crate) fn contours_of(data: &[u8]) -> Result<Vec<Contour>, FormatError> {
    let mut words = Reads::new(Cursor::new(data));
    words.skip(4)?;
    let rings = words.u32_be()?;
    let mut contours = Vec::new();
    for _ in 0..rings {
        let count = words.u32_be()?;
        if u64::from(count) * POINT_BYTES > words.remaining()? {
            let offset = words.position()? as usize;
            return Err(FormatError::MalformedRun { offset });
        }
        let mut points = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let x = words.u32_be()?.cast_signed();
            let y = words.u32_be()?.cast_signed();
            points.push(Vec2::new(x as f32, y as f32) / 1000.0);
        }
        if points.len() > 1 && points.first() == points.last() {
            points.pop();
        }
        contours.extend(Contour::from_points(points));
    }
    Ok(contours)
}
