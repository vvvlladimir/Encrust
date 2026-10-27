//! A layer as the container holds it: its area, then its rings as points in micrometres
//! from the panel's corner. See `docs/formats/cxline.md`.

use core_format::FormatError;
use core_raster::{Ring, double_area_px};
use core_slicer::Contour;
use glam::Vec2;

use crate::v3::blocks::PAGE_BREAK;

/// Bytes of one point: two signed words.
pub(crate) const POINT_BYTES: u64 = 8;

/// The area field of a layer: what it lights in square millimetres, times a thousand.
pub(crate) fn area_field(rings: &[Ring], pitch_mm: (f32, f32)) -> u32 {
    let double_px: i64 = rings.iter().map(double_area_px).sum();
    let area_mm2 = double_px as f64 / 2.0 * f64::from(pitch_mm.0) * f64::from(pitch_mm.1);
    (area_mm2 * 1000.0).round().clamp(0.0, f64::from(u32::MAX)) as u32
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
    let mut words = Words { data, at: 4 };
    let rings = words.next()?;
    let mut contours = Vec::new();
    for _ in 0..rings {
        let count = words.next()?;
        if u64::from(count) * POINT_BYTES > (data.len() - words.at) as u64 {
            return Err(FormatError::MalformedRun { offset: words.at });
        }
        let mut points = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let x = words.next()?.cast_signed();
            let y = words.next()?.cast_signed();
            points.push(Vec2::new(x as f32, y as f32) / 1000.0);
        }
        if points.len() > 1 && points.first() == points.last() {
            points.pop();
        }
        contours.extend(Contour::from_points(points));
    }
    Ok(contours)
}

/// Big-endian words read off the front of a slice, refusing to run past it.
struct Words<'a> {
    data: &'a [u8],
    at: usize,
}

impl Words<'_> {
    fn next(&mut self) -> Result<u32, FormatError> {
        let bytes = self
            .data
            .get(self.at..self.at + 4)
            .ok_or(FormatError::MalformedRun { offset: self.at })?;
        self.at += 4;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
}
