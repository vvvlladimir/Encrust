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

/// A corner of the pixel grid.
type Corner = (u32, u32);

/// A lit stretch of one row, `[start, end)` in pixel columns.
type Span = (u32, u32);

/// One directed step along the boundary, material on its left.
type Edge = (Corner, Corner);

/// The rings of one layer, holes included, each closed and without its first corner
/// repeated at the end.
///
/// The work is proportional to the runs of the layer rather than to the panel, which is
/// what keeps a stack of empty 4K layers from costing a second each.
pub(crate) fn rings_of(layer: &LayerRuns) -> Vec<Ring> {
    stitch(boundary_edges(&lit_spans(layer)))
}

/// The lit stretches of every row, in order, with neighbouring ones joined.
fn lit_spans(layer: &LayerRuns) -> Vec<Vec<Span>> {
    let (width, height) = (u64::from(layer.width()), u64::from(layer.height()));
    let mut rows = vec![Vec::new(); height as usize];
    let (mut at, total) = (0u64, width * height);

    for run in layer.runs() {
        let end = (at + u64::from(run.length)).min(total);
        if run.value >= THRESHOLD {
            push_span(&mut rows, width, at, end);
        }
        at = end;
    }
    rows
}

/// Adds the pixels `[from, to)` of the reading order to the rows they fall in.
fn push_span(rows: &mut [Vec<Span>], width: u64, from: u64, to: u64) {
    let mut at = from;
    while at < to {
        let row = at / width;
        let end = to.min((row + 1) * width);
        let (x0, x1) = ((at - row * width) as u32, (end - row * width) as u32);
        match rows[row as usize].last_mut() {
            Some(last) if last.1 == x0 => last.1 = x1,
            _ => rows[row as usize].push((x0, x1)),
        }
        at = end;
    }
}

/// Every step along the boundary, a whole stretch at a time.
///
/// A corner the outline has to turn at is always an end of one of these stretches: a
/// vertical step can only meet a horizontal one where the row above or below changes,
/// which is where the difference below is cut.
fn boundary_edges(rows: &[Vec<Span>]) -> Vec<Edge> {
    let none: [Span; 0] = [];
    let mut edges = Vec::new();

    for (index, spans) in rows.iter().enumerate() {
        let y = index as u32;
        let above = index.checked_sub(1).map_or(&none[..], |row| &rows[row][..]);
        let below = rows.get(index + 1).map_or(&none[..], Vec::as_slice);

        each_difference(spans, above, |(x0, x1)| edges.push(((x0, y), (x1, y))));
        for &(x0, x1) in spans {
            edges.push(((x1, y), (x1, y + 1)));
            edges.push(((x0, y + 1), (x0, y)));
        }
        each_difference(spans, below, |(x0, x1)| {
            edges.push(((x1, y + 1), (x0, y + 1)));
        });
    }
    edges
}

/// The parts of `spans` that no span of `other` covers, in order. Both are sorted and
/// neither holds two spans that touch.
fn each_difference(spans: &[Span], other: &[Span], mut each: impl FnMut(Span)) {
    let mut index = 0;
    for &(start, end) in spans {
        let mut at = start;
        while other.get(index).is_some_and(|cut| cut.1 <= at) {
            index += 1;
        }
        while at < end {
            let Some(&(cut_start, cut_end)) = other.get(index) else {
                break;
            };
            if cut_start >= end {
                break;
            }
            if cut_start > at {
                each((at, cut_start));
            }
            at = at.max(cut_end);
            if cut_end > end {
                break;
            }
            index += 1;
        }
        if at < end {
            each((at, end));
        }
    }
}

/// Follows the steps into closed rings, taking each one once. A ring begins at the first
/// corner reading order finds a step out of, which is what fixes the order they come in.
fn stitch(mut edges: Vec<Edge>) -> Vec<Ring> {
    edges.sort_by_key(|&((x, y), _)| (y, x));
    let mut taken = vec![false; edges.len()];
    let mut rings = Vec::new();

    for index in 0..edges.len() {
        if taken[index] {
            continue;
        }
        let start = edges[index].0;
        let mut ring: Ring = vec![start];
        let mut step = Some(index);
        while let Some(at) = step {
            taken[at] = true;
            let to = edges[at].1;
            if to == start {
                break;
            }
            push_corner(&mut ring, to);
            step = next_step(&edges, &taken, to);
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
    rings
}

/// The first step out of `at` that no ring has taken, found in the sorted steps.
fn next_step(edges: &[Edge], taken: &[bool], at: Corner) -> Option<usize> {
    let from = edges.partition_point(|&((x, y), _)| (y, x) < (at.1, at.0));
    (from..edges.len())
        .take_while(|&index| edges[index].0 == at)
        .find(|&index| !taken[index])
}

/// Adds a corner, dropping the one before it where the two steps run the same way.
fn push_corner(ring: &mut Ring, corner: Corner) {
    if ring.len() >= 2 && collinear(ring[ring.len() - 2], ring[ring.len() - 1], corner) {
        ring.pop();
    }
    ring.push(corner);
}

/// Whether three corners of the grid stand on one horizontal or vertical line.
fn collinear(a: Corner, b: Corner, c: Corner) -> bool {
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
    fn a_step_in_the_row_above_turns_the_outline_where_it_stands() {
        // A two-row L: the wide row below reaches past the narrow row above, so the top
        // of the shape is two stretches with the step between them.
        let pixels = [255, 255, 0, 0, 255, 255, 255, 255];
        let rings = rings_of(&runs_of(4, 2, &pixels));
        assert_eq!(rings.len(), 1);
        assert_eq!(
            rings[0],
            [(0, 0), (2, 0), (2, 1), (4, 1), (4, 2), (0, 2)],
            "the outline turns where the row above ends"
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
    fn a_run_crossing_the_end_of_a_row_is_two_stretches() {
        // One run of six lit pixels over a four-wide panel: the rows share no boundary
        // down the middle, so the shape is one ring and not two.
        let pixels = [0, 0, 255, 255, 255, 255, 255, 255, 0, 0, 0, 0];
        let rings = rings_of(&runs_of(4, 3, &pixels));
        assert_eq!(rings.len(), 1);
        assert_eq!(rings[0], [(2, 0), (4, 0), (4, 2), (0, 2), (0, 1), (2, 1)]);
    }

    #[test]
    fn a_blank_layer_has_no_rings_at_all() {
        assert!(rings_of(&LayerRuns::builder(8, 8).finish()).is_empty());
    }
}
