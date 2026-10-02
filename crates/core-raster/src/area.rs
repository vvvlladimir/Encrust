use crate::scanline::{Deltas, Edge};

/// Deposits the exact area one edge cuts from each pixel of one row.
///
/// Every pixel the edge crosses receives the signed area the edge takes from it, and the
/// running sum taken in `emit_row` carries that along the row, so the solid pixels between
/// two edges cost nothing at all. The sign follows the edge's direction, which is what
/// makes the running sum a winding number rather than a count of edges.
///
/// The formula is the exact-area one every font rasteriser uses; see
/// `docs/design/rasterisation.md` for where it comes from and what it assumes.
pub(crate) fn add_edge_row(deltas: &mut Deltas, width_px: u32, row: u32, edge: &Edge) {
    let top = (row as f32).max(edge.y_min);
    let bottom = ((row + 1) as f32).min(edge.y_max);
    let height = bottom - top;
    if height <= 0.0 {
        return;
    }

    // Clamping X to the panel is what clips the layer to the display: the part of an edge
    // that runs off the side is flattened onto it, which keeps the area a row deposits
    // summing to zero and so keeps the winding closed.
    let width = width_px as f32;
    let x = edge.x_at(top).clamp(0.0, width);
    let x_next = edge.x_at(bottom).clamp(0.0, width);

    spread(deltas, width_px, x, x_next, height * edge.direction as f32);
}

/// Spreads `area` over the pixels of one row that the segment from `x` to `x_next`
/// crosses, giving each the share of it that falls inside that pixel.
///
/// Columns are relative to the row; `emit_row` is what places the row in the layer.
fn spread(deltas: &mut Deltas, width_px: u32, x: f32, x_next: f32, area: f32) {
    let (left, right) = if x < x_next { (x, x_next) } else { (x_next, x) };
    let left_floor = left.floor();
    let right_ceil = right.ceil();
    let (first, last) = (left_floor as u32, right_ceil as u32);

    // A pixel past the end of the row takes no light, so its share is dropped rather than
    // wrapped onto the row below.
    let mut deposit = |column: u32, share: f32| {
        if column < width_px {
            deltas.push((column, share));
        }
    };

    if last <= first + 1 {
        // The segment stays inside one pixel: its area splits between that pixel and the
        // next by where its midpoint sits, which is exact for a straight segment.
        let middle = 0.5f32.mul_add(x + x_next, -left_floor);
        deposit(first, area * (1.0 - middle));
        deposit(first + 1, area * middle);
        return;
    }

    // The segment crosses several pixels. Its slope is constant, so the area it puts in
    // each one is a triangle at either end and a constant slab in between.
    let inverse_run = (right - left).recip();
    let left_fraction = left - left_floor;
    let first_share = 0.5 * inverse_run * (1.0 - left_fraction) * (1.0 - left_fraction);
    let right_fraction = right - right_ceil + 1.0;
    let last_share = 0.5 * inverse_run * right_fraction * right_fraction;

    deposit(first, area * first_share);
    if last == first + 2 {
        deposit(first + 1, area * (1.0 - first_share - last_share));
    } else {
        let second = inverse_run * (1.5 - left_fraction);
        deposit(first + 1, area * (second - first_share));
        for column in first + 2..last - 1 {
            deposit(column, area * inverse_run);
        }
        let before_last = (last - first - 3) as f32 * inverse_run + second;
        deposit(last - 1, area * (1.0 - before_last - last_share));
    }
    deposit(last, area * last_share);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runs::LayerRuns;
    use crate::scanline::emit_row;
    use crate::settings::Grey;

    /// The greys one row comes out as after a segment deposits `area` across it.
    fn row(width_px: u32, x: f32, x_next: f32, area: f32) -> Vec<u8> {
        let mut deltas = Deltas::new();
        spread(&mut deltas, width_px, x, x_next, area);
        let mut out = LayerRuns::builder(width_px, 1);
        emit_row(&mut deltas, width_px, 0, Grey::default(), &mut out);
        out.finish().to_mask().pixels().to_vec()
    }

    #[test]
    fn a_vertical_segment_lights_everything_to_its_right() {
        // A full-height edge at x = 2.0 leaves pixels 2 and 3 fully covered.
        assert_eq!(row(4, 2.0, 2.0, 1.0), vec![0, 0, 255, 255]);
    }

    #[test]
    fn a_segment_inside_one_pixel_shares_its_area_by_its_midpoint() {
        // Midpoint at 2.25 leaves three quarters of pixel 2 to its right.
        let row = row(4, 2.0, 2.5, 1.0);
        assert_eq!(row[2], 191, "three quarters of 255");
        assert_eq!(row[3], 255);
    }

    #[test]
    fn a_segment_crossing_pixels_leaves_a_ramp() {
        // A diagonal from x = 1 to x = 4 across one row: coverage grows to the right.
        let row = row(6, 1.0, 4.0, 1.0);
        assert_eq!(row[0], 0);
        assert!(row[1] < row[2] && row[2] < row[3], "got {row:?}");
        assert_eq!(row[4], 255);
        assert_eq!(row[5], 255);
    }

    #[test]
    fn the_area_a_segment_deposits_does_not_depend_on_its_direction() {
        let rightwards = row(6, 1.3, 4.7, 1.0);
        let leftwards = row(6, 4.7, 1.3, 1.0);
        assert_eq!(rightwards, leftwards);
    }

    #[test]
    fn a_segment_wound_the_other_way_covers_nothing() {
        // A negative winding is what a body wound inward leaves behind: air, not material.
        assert_eq!(row(5, 1.4, 3.6, -1.0), vec![0; 5]);
    }

    #[test]
    fn a_share_past_the_end_of_the_row_is_dropped() {
        let row = row(3, 2.4, 2.4, 1.0);
        assert_eq!(
            row.len(),
            3,
            "nothing wrapped onto a row that does not exist"
        );
        assert_eq!(row[2], 153, "0.6 of the pixel is to the right of the edge");
    }
}
