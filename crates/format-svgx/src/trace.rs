//! Turning an exposure mask back into the polygons a vector container holds.
//!
//! The boundary between a lit pixel and a dark one is a unit step on the pixel grid, so
//! the outline of a layer is the set of those steps stitched into closed rings. See
//! `docs/formats/svgx.md`.

use core_raster::LayerRuns;

/// Grey from which a pixel counts as material. The container carries no grey at all, so a
/// mask shaded by coverage is cut at half a pixel's worth of it.
const THRESHOLD: u8 = 128;

/// One closed ring of the outline, in pixel-grid corners. A ring that encloses material
/// runs one way round and a hole in it the other, which is what tells them apart.
pub(crate) type Ring = Vec<(u32, u32)>;

/// The rings of one layer, holes included, each closed and without its first corner
/// repeated at the end.
pub(crate) fn rings_of(layer: &LayerRuns) -> Vec<Ring> {
    let (width, height) = (layer.width(), layer.height());
    let mask = layer.to_mask();
    let pixels = mask.pixels();
    let lit = |x: u32, y: u32| pixels[(y * width + x) as usize] >= THRESHOLD;

    // A corner of the grid is a vertex, and every boundary step is one directed edge out
    // of it. Material on the left of the step is what makes a ring positive.
    let corners = (width + 1) as usize;
    let mut out: Vec<Vec<(u32, u32)>> = vec![Vec::new(); corners * (height + 1) as usize];
    let mut push = |from: (u32, u32), to: (u32, u32)| {
        out[from.1 as usize * corners + from.0 as usize].push(to);
    };

    for y in 0..height {
        for x in 0..width {
            if !lit(x, y) {
                continue;
            }
            if y == 0 || !lit(x, y - 1) {
                push((x, y), (x + 1, y));
            }
            if x + 1 == width || !lit(x + 1, y) {
                push((x + 1, y), (x + 1, y + 1));
            }
            if y + 1 == height || !lit(x, y + 1) {
                push((x + 1, y + 1), (x, y + 1));
            }
            if x == 0 || !lit(x - 1, y) {
                push((x, y + 1), (x, y));
            }
        }
    }

    stitch(out, corners)
}

/// Follows the directed steps out of each corner into closed rings, taking each step once.
fn stitch(mut out: Vec<Vec<(u32, u32)>>, corners: usize) -> Vec<Ring> {
    let mut rings = Vec::new();
    for index in 0..out.len() {
        while !out[index].is_empty() {
            let start = ((index % corners) as u32, (index / corners) as u32);
            let mut ring: Ring = vec![start];
            let mut at = start;
            while let Some(next) = out[at.1 as usize * corners + at.0 as usize].pop() {
                at = next;
                if at == start {
                    break;
                }
                push_corner(&mut ring, at);
            }
            // A step along the ring that only continues the one before it is not a corner,
            // and the ring's last point can turn out to be one of those.
            if ring.len() > 2 && collinear(ring[ring.len() - 2], ring[ring.len() - 1], ring[0]) {
                ring.pop();
            }
            if ring.len() >= 4 {
                rings.push(ring);
            }
        }
    }
    rings
}

/// Adds a corner, dropping the one before it where the two steps run the same way.
fn push_corner(ring: &mut Ring, corner: (u32, u32)) {
    if ring.len() >= 2 && collinear(ring[ring.len() - 2], ring[ring.len() - 1], corner) {
        ring.pop();
    }
    ring.push(corner);
}

/// Whether three corners of the grid stand on one horizontal or vertical line.
fn collinear(a: (u32, u32), b: (u32, u32), c: (u32, u32)) -> bool {
    (a.0 == b.0 && b.0 == c.0) || (a.1 == b.1 && b.1 == c.1)
}

/// Twice the signed area of a ring, in pixels. Positive encloses material.
pub(crate) fn double_area_px(ring: &Ring) -> i64 {
    let mut total = 0i64;
    for index in 0..ring.len() {
        let (x0, y0) = ring[index];
        let (x1, y1) = ring[(index + 1) % ring.len()];
        total += i64::from(x0) * i64::from(y1) - i64::from(x1) * i64::from(y0);
    }
    total
}

/// The length of a ring in pixels, which is the sum of its axis-aligned steps.
pub(crate) fn length_px(ring: &Ring) -> u64 {
    let mut total = 0u64;
    for index in 0..ring.len() {
        let (x0, y0) = ring[index];
        let (x1, y1) = ring[(index + 1) % ring.len()];
        total += u64::from(x0.abs_diff(x1)) + u64::from(y0.abs_diff(y1));
    }
    total
}

#[cfg(test)]
mod tests {
    use core_raster::LayerMask;

    use super::*;

    fn runs_of(width: u32, height: u32, pixels: &[u8]) -> LayerRuns {
        let mut mask = LayerMask::new(width, height);
        mask.pixels_mut().copy_from_slice(pixels);
        LayerRuns::from_mask(&mask)
    }

    #[test]
    fn one_lit_pixel_is_a_square_of_four_corners() {
        let rings = rings_of(&runs_of(3, 3, &[0, 0, 0, 0, 255, 0, 0, 0, 0]));
        assert_eq!(rings.len(), 1);
        assert_eq!(rings[0], [(1, 1), (2, 1), (2, 2), (1, 2)]);
        assert!(double_area_px(&rings[0]) > 0, "material runs positive");
        assert_eq!(length_px(&rings[0]), 4);
    }

    #[test]
    fn a_rectangle_keeps_only_its_corners() {
        let mut pixels = vec![0u8; 6 * 4];
        for y in 1..3 {
            for x in 1..5 {
                pixels[y * 6 + x] = 255;
            }
        }
        let rings = rings_of(&runs_of(6, 4, &pixels));
        assert_eq!(rings.len(), 1);
        assert_eq!(
            rings[0],
            [(1, 1), (5, 1), (5, 3), (1, 3)],
            "the straight sides carry no corner of their own"
        );
    }

    #[test]
    fn a_hole_is_its_own_ring_the_other_way_round() {
        let mut pixels = vec![255u8; 5 * 5];
        pixels[2 * 5 + 2] = 0;
        let rings = rings_of(&runs_of(5, 5, &pixels));
        assert_eq!(rings.len(), 2);

        let areas: Vec<i64> = rings.iter().map(double_area_px).collect();
        assert!(
            areas.iter().any(|&area| area > 0) && areas.iter().any(|&area| area < 0),
            "{areas:?}: the hole encloses nothing and so runs the other way"
        );
    }

    #[test]
    fn two_islands_are_two_rings() {
        let pixels = [255, 0, 255, 0, 0, 0, 255, 0, 255];
        let rings = rings_of(&runs_of(3, 3, &pixels));
        assert_eq!(rings.len(), 4, "the corners touch nothing");
    }

    #[test]
    fn a_grey_below_the_cut_is_not_material() {
        let rings = rings_of(&runs_of(3, 1, &[0, 100, 0]));
        assert!(rings.is_empty(), "a tenth-lit pixel is not a wall");
        assert_eq!(rings_of(&runs_of(3, 1, &[0, 128, 0])).len(), 1);
    }

    #[test]
    fn a_blank_layer_has_no_rings_at_all() {
        assert!(rings_of(&LayerRuns::builder(8, 8).finish()).is_empty());
    }
}
