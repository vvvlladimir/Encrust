use crate::LayerMask;

/// A stretch of neighbouring pixels sharing one grey value.
///
/// Lengths are pixel counts in reading order, so a run may cross the end of a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Run {
    pub length: u32,
    pub value: u8,
}

/// Keeps layer cost proportional to its contours, not the panel (`docs/design/rasterisation.md`).
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

    /// Pixels the layer lights at any grey.
    pub fn lit_px(&self) -> u64 {
        lit_px(&self.runs)
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

    /// The brighter of two same-size layers, pixel by pixel; combines planes sampled inside one
    /// layer. A layer of another size is returned unchanged.
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

    /// The same layer with its rows in the opposite order, the last first: the plate seen
    /// from its other edge.
    #[must_use]
    pub fn rows_reversed(&self) -> Self {
        let mut rows: Vec<Vec<Run>> = Vec::with_capacity(self.height as usize);
        let mut row = Vec::new();
        let mut filled = 0;
        for run in &self.runs {
            let mut left = run.length;
            while left > 0 && self.width > 0 {
                let take = left.min(self.width - filled);
                row.push(Run {
                    length: take,
                    value: run.value,
                });
                (filled, left) = (filled + take, left - take);
                if filled == self.width {
                    rows.push(std::mem::take(&mut row));
                    filled = 0;
                }
            }
        }
        let mut builder = Self::builder(self.width, self.height);
        for run in rows.iter().rev().flatten() {
            builder.push(run.length, run.value);
        }
        builder.finish()
    }
}

/// Collects the runs of one layer in reading order. Neighbouring runs of the same value merge as
/// they arrive, so a pixel at a time is fine.
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
        push_run(&mut self.runs, length, value);
        self.emitted += length;
    }

    /// Leaves everything up to pixel `index` dark. A position already passed is ignored,
    /// which is what lets a caller skip whole rows without tracking where it is.
    pub fn pad_to(&mut self, index: u32) {
        self.push(index.saturating_sub(self.emitted), 0);
    }

    /// Fills whatever is left with dark pixels and closes the layer. The printer decodes into an
    /// uncleared buffer, so short runs would print the previous layer's pixels.
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

/// Pixels `runs` light at any grey.
pub fn lit_px(runs: &[Run]) -> u64 {
    runs.iter()
        .filter(|run| run.value > 0)
        .map(|run| u64::from(run.length))
        .sum()
}

/// Appends `length` pixels of `value` to `runs`, joining the last run when it has that value.
pub fn push_run(runs: &mut Vec<Run>, length: u32, value: u8) {
    match runs.last_mut() {
        Some(last) if last.value == value => last.length += length,
        _ => runs.push(Run { length, value }),
    }
}

/// Pixels of a panel. `RasterSettings::validate` rejects one that does not fit in a
/// `u32`, so the product is exact here.
fn pixel_count(width: u32, height: u32) -> u32 {
    (u64::from(width) * u64::from(height)).min(u64::from(u32::MAX)) as u32
}

/// Pixels compared at a time when looking for the end of a run: a foreign mask is mostly one
/// dark run, and scanning it a byte at a time costs more than rasterising it did.
const SCAN_STRIDE: usize = 16;

/// What is left of `run` after `taken` pixels of it, or `None` when it is used up.
fn remainder(run: Run, taken: u32) -> Option<Run> {
    (run.length > taken).then(|| Run {
        length: run.length - taken,
        value: run.value,
    })
}

/// How many pixels from the front of `pixels` carry `value`.
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
mod tests;
