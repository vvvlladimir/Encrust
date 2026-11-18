//! How shut in a pixel's surface is, from the depths around it: crevices and the folds of a
//! figure darken, a plane however steep stays open.

use super::raster::Surfaces;

/// Most of the light a fully shut-in pixel loses.
pub(super) const MAX_OCCLUSION: f32 = 0.33;

/// Radii looked out to, as fractions of the image width: wide enough that a fold between
/// limbs darkens and the pores of a sculpted surface do not.
const RADII: [f32; 2] = [0.014, 0.022];

/// Directions sampled, each with its opposite.
const DIRECTIONS: [(i64, i64); 4] = [(1, 0), (0, 1), (1, 1), (1, -1)];

/// How deep a crevice, in radii, shuts a pixel in fully, and past how many radii a nearer
/// surface is another part in front rather than the sides of a fold.
const FULL_DEPTH: f32 = 0.6;
const FARTHEST: f32 = 3.0;

/// Share of the light around `(x, y)` its surroundings block, `0..=MAX_OCCLUSION`: per
/// pair of opposite samples, how far their mean stands nearer the eye than the pixel.
pub(super) fn occlusion(surfaces: &Surfaces, x: i64, y: i64, pixel_mm: f32) -> f32 {
    let Some(here) = surfaces.depth(x, y) else {
        return 0.0;
    };
    let mut total = 0.0;
    let mut count = 0u32;
    for share in RADII {
        let radius_px = (share * surfaces.width_px as f32).round().max(1.0);
        let reach = radius_px as i64;
        let radius_mm = radius_px * pixel_mm;
        for (dx, dy) in DIRECTIONS {
            let (Some(one), Some(other)) = (
                surfaces.depth(x + dx * reach, y + dy * reach),
                surfaces.depth(x - dx * reach, y - dy * reach),
            ) else {
                continue;
            };
            let rise = (here - f32::midpoint(one, other)) / radius_mm;
            count += 1;
            if rise < FARTHEST {
                total += (rise / FULL_DEPTH).clamp(0.0, 1.0);
            }
        }
    }
    if count == 0 {
        return 0.0;
    }
    MAX_OCCLUSION * total / count as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 100 by 100 field of depths, `depth(x, y)` in pixel widths of 1 mm.
    fn field(depth: impl Fn(i64, i64) -> f32) -> Surfaces {
        let mut surfaces = Surfaces::new(100, 100);
        for y in 0..100 {
            for x in 0..100 {
                surfaces.depths[(y * 100 + x) as usize] = depth(x, y);
            }
        }
        surfaces
    }

    #[test]
    fn a_steep_plane_is_open() {
        let surfaces = field(|x, y| 3.0 * x as f32 + 2.0 * y as f32);
        assert_eq!(occlusion(&surfaces, 50, 50, 1.0), 0.0);
    }

    #[test]
    fn the_floor_of_a_groove_is_shut_in() {
        let surfaces = field(|x, _| -((x - 50).abs() as f32));
        let floor = occlusion(&surfaces, 50, 50, 1.0);
        assert!(
            floor > 0.5 * MAX_OCCLUSION,
            "the bottom of a right-angled groove loses light, got {floor}"
        );
    }

    #[test]
    fn a_ridge_is_open() {
        let surfaces = field(|x, _| (x - 50).abs() as f32);
        assert_eq!(occlusion(&surfaces, 50, 50, 1.0), 0.0);
    }
}
