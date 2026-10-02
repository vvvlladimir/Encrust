use crate::LayerMask;

/// A stretch of neighbouring pixels sharing one grey value.
///
/// Lengths are pixel counts in reading order, so a run may cross the end of a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Run {
    pub length: u32,
    pub value: u8,
}

/// One layer as runs of identical pixels, row-major from the top-left of the LCD.
///
/// This is what a rasteriser produces and what a sliced-file format consumes. Every MSLA
/// format either encodes runs directly, as the `.goo` and `.ctb` families do, or expands
/// them into pixels with [`LayerRuns::to_mask`]. Working in runs is also what keeps the
/// cost of a layer proportional to its contours rather than to the panel; see
/// `docs/design/rasterisation.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerRuns {
    width: u32,
    height: u32,
    runs: Vec<Run>,
}

impl LayerRuns {
    /// Starts an all-dark layer of `width` x `height` pixels, to be filled in reading
    /// order.
    pub fn builder(width: u32, height: u32) -> RunsBuilder {
        RunsBuilder {
            width,
            height,
            runs: Vec::new(),
            emitted: 0,
        }
    }

    /// Run-length encodes a dense mask, for pixels that came from somewhere other than
    /// the rasteriser.
    pub fn from_mask(mask: &LayerMask) -> Self {
        let pixels = mask.pixels();
        let mut builder = Self::builder(mask.width(), mask.height());

        let mut start = 0;
        while start < pixels.len() {
            let value = pixels[start];
            let length = run_length(&pixels[start..], value);
            builder.push(length as u32, value);
            start += length;
        }
        builder.finish()
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn runs(&self) -> &[Run] {
        &self.runs
    }

    /// Pixels the layer covers, which is what the runs add up to.
    pub fn pixel_count(&self) -> u32 {
        pixel_count(self.width, self.height)
    }

    /// Expands the runs into pixels, for a format or a preview that wants a bitmap.
    pub fn to_mask(&self) -> LayerMask {
        let mut mask = LayerMask::new(self.width, self.height);
        let total = mask.pixels().len();
        let mut at = 0usize;
        for run in &self.runs {
            let end = (at + run.length as usize).min(total);
            mask.pixels_mut()[at..end].fill(run.value);
            at = end;
        }
        mask
    }

    /// Total exposure expressed as an area, in pixels: a fully lit pixel counts as one,
    /// a half-lit one as a half.
    pub fn coverage(&self) -> f32 {
        let total: u64 = self
            .runs
            .iter()
            .map(|run| u64::from(run.length) * u64::from(run.value))
            .sum();
        total as f32 / 255.0
    }

    /// The brighter of two layers of the same size, pixel by pixel.
    ///
    /// This is how the planes sampled inside one layer are combined: a feature standing on
    /// one plane and not the next keeps its own coverage instead of being averaged away.
    /// A layer of another size is returned unchanged, since there is nothing to line up.
    #[must_use]
    pub fn brightest_of(&self, other: &Self) -> Self {
        if self.width != other.width || self.height != other.height {
            return self.clone();
        }
        let mut builder = Self::builder(self.width, self.height);
        let (mut mine, mut theirs) = (self.runs.iter().copied(), other.runs.iter().copied());
        let (mut here, mut there) = (mine.next(), theirs.next());

        while let (Some(a), Some(b)) = (here, there) {
            let length = a.length.min(b.length);
            builder.push(length, a.value.max(b.value));
            here = remainder(a, length).or_else(|| mine.next());
            there = remainder(b, length).or_else(|| theirs.next());
        }
        builder.finish()
    }

    pub fn is_blank(&self) -> bool {
        self.runs.iter().all(|run| run.value == 0)
    }
}

/// Collects the runs of one layer in reading order.
///
/// Neighbouring runs of the same value are merged as they arrive, so a caller may push a
/// pixel at a time without producing a run per pixel.
#[derive(Debug)]
pub struct RunsBuilder {
    width: u32,
    height: u32,
    runs: Vec<Run>,
    emitted: u32,
}

impl RunsBuilder {
    /// Appends `length` pixels of `value`. Anything past the end of the layer is dropped.
    pub fn push(&mut self, length: u32, value: u8) {
        let length = length.min(self.remaining());
        if length == 0 {
            return;
        }
        match self.runs.last_mut() {
            Some(last) if last.value == value => last.length += length,
            _ => self.runs.push(Run { length, value }),
        }
        self.emitted += length;
    }

    /// Leaves everything up to pixel `index` dark. A position already passed is ignored,
    /// which is what lets a caller skip whole rows without tracking where it is.
    pub fn pad_to(&mut self, index: u32) {
        self.push(index.saturating_sub(self.emitted), 0);
    }

    /// Fills whatever is left with dark pixels and closes the layer.
    ///
    /// The printer decodes a layer into a buffer it does not clear first, so runs that
    /// stop short of the last pixel print whatever the previous layer left there.
    pub fn finish(mut self) -> LayerRuns {
        self.push(self.remaining(), 0);
        LayerRuns {
            width: self.width,
            height: self.height,
            runs: self.runs,
        }
    }

    fn remaining(&self) -> u32 {
        pixel_count(self.width, self.height) - self.emitted
    }
}

/// Pixels of a panel. `RasterSettings::validate` rejects one that does not fit in a
/// `u32`, so the product is exact here.
fn pixel_count(width: u32, height: u32) -> u32 {
    (u64::from(width) * u64::from(height)).min(u64::from(u32::MAX)) as u32
}

/// Pixels compared at a time when looking for the end of a run. Most of a mask handed in
/// from outside is one long dark run, and a byte-at-a-time scan of a panel-sized mask
/// costs more than rasterising it did.
const SCAN_STRIDE: usize = 16;

/// How many pixels from the front of `pixels` carry `value`.
/// What is left of `run` after `taken` pixels of it, or `None` when it is used up.
fn remainder(run: Run, taken: u32) -> Option<Run> {
    (run.length > taken).then(|| Run {
        length: run.length - taken,
        value: run.value,
    })
}

fn run_length(pixels: &[u8], value: u8) -> usize {
    let (blocks, _) = pixels.as_chunks::<SCAN_STRIDE>();
    let run = [value; SCAN_STRIDE];
    let whole = blocks.iter().take_while(|block| **block == run).count() * SCAN_STRIDE;

    whole
        + pixels[whole..]
            .iter()
            .take_while(|&&pixel| pixel == value)
            .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runs_of(width: u32, values: &[(u32, u8)]) -> LayerRuns {
        let mut builder = LayerRuns::builder(width, 1);
        for &(length, value) in values {
            builder.push(length, value);
        }
        builder.finish()
    }

    #[test]
    fn the_brighter_pixel_of_two_layers_wins() {
        let a = runs_of(6, &[(2, 255), (4, 0)]);
        let b = runs_of(6, &[(1, 0), (2, 100), (3, 0)]);

        let both = a.brightest_of(&b);
        assert_eq!(both.to_mask().pixels(), &[255, 255, 100, 0, 0, 0]);
        assert_eq!(
            both.to_mask().pixels(),
            b.brightest_of(&a).to_mask().pixels(),
            "the brighter of two does not depend which is asked"
        );
    }

    #[test]
    fn a_union_covers_the_whole_panel_however_the_runs_line_up() {
        let a = runs_of(4, &[(4, 0)]);
        let b = runs_of(4, &[(1, 9), (1, 8), (1, 7), (1, 6)]);
        assert_eq!(a.brightest_of(&b).pixel_count(), 4);
        assert_eq!(a.brightest_of(&b).to_mask().pixels(), &[9, 8, 7, 6]);
    }

    #[test]
    fn a_layer_nobody_wrote_to_is_one_dark_run() {
        let layer = LayerRuns::builder(4, 2).finish();
        assert_eq!(
            layer.runs(),
            &[Run {
                length: 8,
                value: 0
            }]
        );
        assert!(layer.is_blank());
    }

    #[test]
    fn neighbouring_runs_of_one_value_are_merged() {
        let mut builder = LayerRuns::builder(4, 1);
        builder.push(1, 200);
        builder.push(2, 200);
        let layer = builder.finish();

        assert_eq!(
            layer.runs(),
            &[
                Run {
                    length: 3,
                    value: 200
                },
                Run {
                    length: 1,
                    value: 0
                }
            ]
        );
    }

    #[test]
    fn padding_skips_to_a_position_and_never_backwards() {
        let mut builder = LayerRuns::builder(4, 2);
        builder.pad_to(5);
        builder.push(1, 255);
        // Already past pixel 3, so this adds nothing.
        builder.pad_to(3);
        builder.push(1, 255);
        let layer = builder.finish();

        assert_eq!(
            layer.to_mask().pixels(),
            &[0, 0, 0, 0, 0, 255, 255, 0],
            "the two lit pixels sit where they were pushed"
        );
    }

    #[test]
    fn runs_past_the_end_of_the_layer_are_dropped() {
        let mut builder = LayerRuns::builder(2, 1);
        builder.push(100, 255);
        let layer = builder.finish();

        assert_eq!(
            layer.runs(),
            &[Run {
                length: 2,
                value: 255
            }]
        );
        assert_eq!(layer.to_mask().pixels(), &[255, 255]);
    }

    #[test]
    fn a_mask_survives_the_trip_through_runs() {
        let mut mask = LayerMask::new(5, 2);
        mask.pixels_mut()
            .copy_from_slice(&[0, 0, 7, 7, 7, 7, 255, 0, 0, 0]);
        let layer = LayerRuns::from_mask(&mask);

        assert_eq!(layer.to_mask(), mask);
        assert_eq!(layer.runs().len(), 4, "got {:?}", layer.runs());
    }

    #[test]
    fn a_run_longer_than_the_scan_stride_is_found_in_one_piece() {
        let mut mask = LayerMask::new(SCAN_STRIDE as u32 * 3 + 5, 1);
        mask.pixels_mut().fill(9);
        let layer = LayerRuns::from_mask(&mask);

        assert_eq!(
            layer.runs(),
            &[Run {
                length: SCAN_STRIDE as u32 * 3 + 5,
                value: 9
            }]
        );
    }

    #[test]
    fn coverage_counts_a_half_lit_pixel_as_a_half() {
        let mut builder = LayerRuns::builder(2, 2);
        builder.push(1, 255);
        builder.push(1, 128);
        let layer = builder.finish();

        assert!((layer.coverage() - 1.502).abs() < 1e-3);
    }

    #[test]
    fn coverage_of_a_panel_sized_layer_does_not_overflow() {
        let mut builder = LayerRuns::builder(8520, 4320);
        builder.push(8520 * 4320, 255);
        let layer = builder.finish();

        let expected = (8520u64 * 4320) as f32;
        assert!((layer.coverage() - expected).abs() < expected * 1e-6);
    }
}
