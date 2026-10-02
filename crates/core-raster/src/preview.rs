use crate::{LayerMask, LayerRuns};

/// Shrinks a rasterised layer to at most `max_width_px` x `max_height_px` pixels.
///
/// A block of source pixels becomes the brightest pixel it holds rather than their
/// average: a wall one pixel wide is what a preview exists to show, and averaging a
/// 5760 x 3600 panel down to a screen would fade it out of the picture. Both axes shrink
/// by the same factor, so the mask keeps the panel's pixel aspect.
pub fn downsample(runs: &LayerRuns, max_width_px: u32, max_height_px: u32) -> LayerMask {
    let (width, height) = (runs.width(), runs.height());
    if width == 0 || height == 0 {
        return LayerMask::new(0, 0);
    }

    let factor = shrink_factor(width, height, max_width_px, max_height_px);
    let mut mask = LayerMask::new(width.div_ceil(factor), height.div_ceil(factor));

    let mut at = 0u64;
    for run in runs.runs() {
        let end = at + u64::from(run.length);
        if run.value > 0 {
            brighten(&mut mask, factor, u64::from(width), at..end, run.value);
        }
        at = end;
    }
    mask
}

/// The `width_px` x `height_px` pixels of a layer from `(x_px, y_px)`, at full resolution.
///
/// What a shrunk preview cannot show, such as the grey of an edge, a magnifier shows here.
/// Pixels past the panel are dark.
pub fn crop(runs: &LayerRuns, x_px: u32, y_px: u32, width_px: u32, height_px: u32) -> LayerMask {
    let mut mask = LayerMask::new(width_px, height_px);
    let source_width = u64::from(runs.width());
    if source_width == 0 {
        return mask;
    }
    let (left, right) = (u64::from(x_px), u64::from(x_px) + u64::from(width_px));
    let (top, bottom) = (u64::from(y_px), u64::from(y_px) + u64::from(height_px));
    let first = top * source_width;
    let past = bottom * source_width;

    let mut at = 0u64;
    for run in runs.runs() {
        let end = at + u64::from(run.length);
        let mut pixel = at.max(first);
        at = end;
        if run.value == 0 {
            continue;
        }
        while pixel < end.min(past) {
            let (row, column) = (pixel / source_width, pixel % source_width);
            let stop = end.min((row + 1) * source_width) - row * source_width;
            let (from, to) = (column.max(left), stop.min(right));
            if from < to {
                let start = ((row - top) * u64::from(width_px) + from - left) as usize;
                mask.pixels_mut()[start..start + (to - from) as usize].fill(run.value);
            }
            pixel = (row + 1) * source_width;
        }
    }
    mask
}

/// Pixels of the panel that collapse into one pixel of the preview, along each axis.
///
/// Public so that a caller can say what it is showing: a mask shrunk by four is a quarter
/// of the panel's resolution, and the screen has to admit that.
pub fn shrink_factor(width: u32, height: u32, max_width_px: u32, max_height_px: u32) -> u32 {
    let by_x = width.div_ceil(max_width_px.max(1));
    let by_y = height.div_ceil(max_height_px.max(1));
    by_x.max(by_y).max(1)
}

/// Raises every preview pixel the source range `pixels` touches to at least `value`.
///
/// The range is in reading order and may cross rows, so it is walked a row at a time:
/// two ends of one run belong to opposite sides of the image.
fn brighten(
    mask: &mut LayerMask,
    factor: u32,
    source_width: u64,
    pixels: std::ops::Range<u64>,
    value: u8,
) {
    let target_width = u64::from(mask.width());
    let mut at = pixels.start;

    while at < pixels.end {
        let row = at / source_width;
        let row_end = (row + 1) * source_width;
        let run_end = pixels.end.min(row_end);

        let target_row = row / u64::from(factor);
        let first = (at % source_width) / u64::from(factor);
        let last = ((run_end - 1) % source_width) / u64::from(factor);
        let start = (target_row * target_width + first) as usize;
        let end = (target_row * target_width + last) as usize;
        for pixel in &mut mask.pixels_mut()[start..=end] {
            *pixel = (*pixel).max(value);
        }

        at = run_end;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a layer from one grey value per pixel, in reading order.
    fn layer(width: u32, height: u32, pixels: &[u8]) -> LayerRuns {
        let mut builder = LayerRuns::builder(width, height);
        for &value in pixels {
            builder.push(1, value);
        }
        builder.finish()
    }

    #[test]
    fn a_layer_that_already_fits_is_returned_pixel_for_pixel() {
        let runs = layer(2, 2, &[0, 255, 128, 0]);
        let mask = downsample(&runs, 16, 16);
        assert_eq!(mask.width(), 2);
        assert_eq!(mask.height(), 2);
        assert_eq!(mask.pixels(), &[0, 255, 128, 0]);
    }

    #[test]
    fn one_lit_pixel_survives_the_shrink() {
        // A single pixel at (x = 2, y = 1) of a 4 x 4 layer lands in block (1, 0).
        let mut pixels = [0u8; 16];
        pixels[4 + 2] = 255;
        let mask = downsample(&layer(4, 4, &pixels), 2, 2);

        assert_eq!(mask.width(), 2);
        assert_eq!(mask.height(), 2);
        assert_eq!(mask.pixels(), &[0, 255, 0, 0]);
    }

    #[test]
    fn the_brightest_pixel_of_a_block_wins() {
        let mask = downsample(&layer(2, 2, &[10, 200, 30, 40]), 1, 1);
        assert_eq!(mask.pixels(), &[200], "the block keeps its brightest pixel");
    }

    #[test]
    fn a_run_crossing_a_row_lights_both_ends() {
        // Pixels 3..=4 of a 4 x 2 layer: the last of the top row and the first of the
        // bottom one, which are opposite corners of the image.
        let mut builder = LayerRuns::builder(4, 2);
        builder.pad_to(3);
        builder.push(2, 255);
        let mask = downsample(&builder.finish(), 2, 2);

        assert_eq!(mask.width(), 2);
        assert_eq!(mask.height(), 1);
        assert_eq!(mask.pixels(), &[255, 255]);
    }

    #[test]
    fn a_size_that_does_not_divide_keeps_the_leftover_pixels() {
        // 5 pixels shrunk by three: two blocks, the second holding the two left over.
        let mut pixels = [0u8; 25];
        pixels[4 * 5 + 4] = 255;
        let mask = downsample(&layer(5, 5, &pixels), 2, 2);

        assert_eq!((mask.width(), mask.height()), (2, 2));
        assert_eq!(mask.pixels(), &[0, 0, 0, 255], "the far corner stays lit");
    }

    #[test]
    fn a_blank_layer_shrinks_to_a_blank_mask() {
        let mask = downsample(&LayerRuns::builder(64, 32).finish(), 8, 8);
        assert_eq!((mask.width(), mask.height()), (8, 4));
        assert!(mask.is_blank());
    }

    #[test]
    fn a_crop_keeps_every_pixel_it_covers() {
        let pixels: Vec<u8> = (0..20).collect();
        let mask = crop(&layer(5, 4, &pixels), 1, 1, 3, 2);
        assert_eq!(mask.pixels(), &[6, 7, 8, 11, 12, 13]);
    }

    #[test]
    fn a_crop_past_the_panel_is_dark_there() {
        let mask = crop(&layer(2, 2, &[1, 2, 3, 4]), 1, 1, 3, 2);
        assert_eq!(mask.pixels(), &[4, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn a_layer_with_no_pixels_has_nothing_to_show() {
        let mask = downsample(&LayerRuns::builder(0, 0).finish(), 8, 8);
        assert_eq!((mask.width(), mask.height()), (0, 0));
    }
}
