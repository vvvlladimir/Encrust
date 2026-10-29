//! Where a layer is lit, in pixels and in millimetres from the panel's centre: what the
//! table container's model block and the archive's scene entry state.

use core_format::PrintJob;
use core_raster::LayerRuns;

/// First and last lit column and row of a layer, or `None` for a dark one.
pub(crate) fn lit_box(layer: &LayerRuns) -> Option<[u32; 4]> {
    let width = u64::from(layer.width());
    let mut found: Option<[u32; 4]> = None;
    let mut at = 0_u64;
    for run in layer.runs() {
        let length = u64::from(run.length);
        if run.value > 0 && length > 0 {
            let (first, last) = (at, at + length - 1);
            let (row_0, row_1) = ((first / width) as u32, (last / width) as u32);
            let (column_0, column_1) = if row_0 == row_1 {
                ((first % width) as u32, (last % width) as u32)
            } else {
                (0, (width - 1) as u32)
            };
            found = Some(union(found, [column_0, row_0, column_1, row_1]));
        }
        at += length;
    }
    found
}

/// The smallest box holding both, either of which may be absent.
pub(crate) fn union<T: PartialOrd + Copy>(a: Option<[T; 4]>, b: [T; 4]) -> [T; 4] {
    let Some([x0, y0, x1, y1]) = a else {
        return b;
    };
    let min = |p: T, q: T| if q < p { q } else { p };
    let max = |p: T, q: T| if q > p { q } else { p };
    [min(x0, b[0]), min(y0, b[1]), max(x1, b[2]), max(y1, b[3])]
}

/// A lit box in millimetres from the panel's centre: left, top, right, bottom. The last
/// lit pixel's far edge is where the box ends.
pub(crate) fn box_mm(job: &PrintJob, [x0, y0, x1, y1]: [u32; 4]) -> [f32; 4] {
    let pitch = job.raster.pitch;
    let (half_x, half_y) = (
        job.raster.width_px as f32 * pitch.x / 2.0,
        job.raster.height_px as f32 * pitch.y / 2.0,
    );
    [
        x0 as f32 * pitch.x - half_x,
        y0 as f32 * pitch.y - half_y,
        (x1 + 1) as f32 * pitch.x - half_x,
        (y1 + 1) as f32 * pitch.y - half_y,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dark_layer_has_no_box() {
        let mut builder = LayerRuns::builder(4, 2);
        builder.push(8, 0);
        assert_eq!(lit_box(&builder.finish()), None);
    }

    #[test]
    fn a_run_across_rows_spans_the_whole_width() {
        let mut builder = LayerRuns::builder(4, 3);
        builder.push(3, 0);
        builder.push(3, 255);
        builder.push(6, 0);
        assert_eq!(lit_box(&builder.finish()), Some([0, 0, 3, 1]));
    }
}
