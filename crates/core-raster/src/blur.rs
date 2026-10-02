use std::iter;
use std::ops::Range;

use crate::runs::{LayerRuns, RunsBuilder};

/// A step in a pixel row: from `x` the value holds until the next step or the end of the
/// row. A row with no steps is dark.
type Step = (u32, u32);

impl LayerRuns {
    /// The layer under a box filter `2 · radius_px + 1` pixels wide in each direction.
    ///
    /// Worked on the steps between runs rather than on pixels, so the cost follows the
    /// edges and not the panel; see `docs/design/rasterisation.md`.
    #[must_use]
    pub fn blurred(&self, radius_px: u8) -> Self {
        let Some(lit) = lit_rows(self).filter(|_| radius_px > 0) else {
            return self.clone();
        };
        let (width, height, radius) = (self.width(), self.height(), u32::from(radius_px));
        let rows = lit.start.saturating_sub(radius)..(lit.end + radius).min(height);
        let split = rows_of(self, rows.clone());
        let mut across = Rows::with_capacity(rows.len(), split.steps.len() * 2);
        for row in 0..rows.len() {
            box_row(split.row(row), width, radius, &mut across.steps);
            across.close_row();
        }

        let mut out = LayerRuns::builder(width, height);
        let mut down = Down::new(radius);
        for y in rows.clone() {
            let lo = y.saturating_sub(radius).max(rows.start) - rows.start;
            let hi = (y + radius + 1).min(rows.end) - rows.start;
            let window = (lo..hi).map(|row| across.row(row as usize));
            down.emit(window, width, y * width, &mut out);
        }
        out.finish()
    }
}

/// Rows of steps stored end to end, so a layer costs two allocations rather than one a row.
struct Rows {
    steps: Vec<Step>,
    ends: Vec<usize>,
}

impl Rows {
    fn with_capacity(rows: usize, steps: usize) -> Self {
        Self {
            steps: Vec::with_capacity(steps),
            ends: Vec::with_capacity(rows),
        }
    }

    fn start(&self) -> usize {
        self.ends.last().copied().unwrap_or(0)
    }

    fn row(&self, index: usize) -> &[Step] {
        let start = index.checked_sub(1).map_or(0, |before| self.ends[before]);
        &self.steps[start..self.ends[index]]
    }

    /// Ends the row being written, dropping its steps if none of them is lit.
    fn close_row(&mut self) {
        let start = self.start();
        if self.steps[start..].iter().all(|&(_, value)| value == 0) {
            self.steps.truncate(start);
        }
        self.ends.push(self.steps.len());
    }
}

/// The first and one past the last row holding a lit pixel, or `None` for a blank layer.
fn lit_rows(layer: &LayerRuns) -> Option<Range<u32>> {
    let width = layer.width();
    let mut at = 0u32;
    let (mut first, mut last) = (None, 0);
    for run in layer.runs() {
        if run.value != 0 {
            first.get_or_insert(at / width);
            last = (at + run.length - 1) / width + 1;
        }
        at += run.length;
    }
    first.map(|first| first..last)
}

/// The rows of `range` as steps, splitting runs where they wrap from one row to the next.
fn rows_of(layer: &LayerRuns, range: Range<u32>) -> Rows {
    let width = layer.width();
    let (first, past) = (range.start * width, range.end * width);
    let mut rows = Rows::with_capacity(range.len(), layer.runs().len() + range.len());
    let mut current = range.start;
    let mut at = 0u32;
    for run in layer.runs() {
        let mut x = at.max(first);
        at += run.length;
        while x < at.min(past) {
            let row = x / width;
            for _ in current..row {
                rows.close_row();
            }
            current = row;
            let start = rows.start();
            push_step(&mut rows.steps, start, x % width, u32::from(run.value));
            x = at.min((row + 1) * width);
        }
    }
    for _ in current..range.end {
        rows.close_row();
    }
    rows
}

/// Value of a step row at pixel `x`, dark past either end.
fn value_at(steps: &[Step], width: u32, x: i64) -> u32 {
    if x < 0 || x >= i64::from(width) {
        return 0;
    }
    let index = steps.partition_point(|&(from, _)| i64::from(from) <= x);
    index.checked_sub(1).map_or(0, |i| steps[i].1)
}

/// Reads a step row at positions that mostly move forward, without searching it again.
struct Reader<'a> {
    steps: &'a [Step],
    width: u32,
    next: usize,
}

impl<'a> Reader<'a> {
    fn new(steps: &'a [Step], width: u32) -> Self {
        Self {
            steps,
            width,
            next: 0,
        }
    }

    fn at(&mut self, x: i64) -> u32 {
        if x < 0 || x >= i64::from(self.width) {
            return 0;
        }
        let behind = self
            .next
            .checked_sub(1)
            .is_some_and(|last| i64::from(self.steps[last].0) > x);
        if behind {
            self.next = 0;
        }
        while self
            .steps
            .get(self.next)
            .is_some_and(|&(from, _)| i64::from(from) <= x)
        {
            self.next += 1;
        }
        self.next.checked_sub(1).map_or(0, |i| self.steps[i].1)
    }
}

/// Appends a row's sums over `[x - radius, x + radius]`, left unscaled, to `out`.
///
/// Away from a step the window holds one value, so only pixels within `radius` of a step
/// are summed one at a time; every other stretch is its value times the window.
fn box_row(steps: &[Step], width: u32, radius: u32, out: &mut Vec<Step>) {
    if steps.is_empty() {
        return;
    }
    let (row, span, reach) = (out.len(), 2 * radius + 1, i64::from(radius));
    let (mut first, mut lead, mut trail) = (
        Reader::new(steps, width),
        Reader::new(steps, width),
        Reader::new(steps, width),
    );
    let mut x = 0u32;
    for (lo, hi) in near_steps(steps, width, radius) {
        if x < lo {
            push_step(out, row, x, value_at(steps, width, i64::from(x)) * span);
        }
        let (lo_x, hi_x) = (i64::from(lo), i64::from(hi));
        let mut sum: u32 = (lo_x - reach..=lo_x + reach).map(|x| first.at(x)).sum();
        for x in lo_x..hi_x {
            push_step(out, row, x as u32, sum);
            sum = sum + lead.at(x + reach + 1) - trail.at(x - reach);
        }
        x = hi;
    }
    if x < width {
        push_step(out, row, x, value_at(steps, width, i64::from(x)) * span);
    }
}

/// The pixels whose window straddles a step, as merged ranges clipped to the row.
///
/// The panel's own sides count as steps, since the window reads dark past them.
fn near_steps(steps: &[Step], width: u32, radius: u32) -> impl Iterator<Item = (u32, u32)> {
    let mut edges = steps
        .iter()
        .map(|&(from, _)| from)
        .chain(iter::once(width))
        .peekable();
    iter::from_fn(move || {
        let edge = edges.next()?;
        let (lo, mut hi) = (edge.saturating_sub(radius), (edge + radius).min(width));
        while let Some(&next) = edges.peek() {
            if next.saturating_sub(radius) > hi {
                break;
            }
            hi = hi.max((next + radius).min(width));
            edges.next();
        }
        Some((lo, hi))
    })
    .filter(|(lo, hi)| lo < hi)
}

/// Adds a step to the row that starts at `row` in `steps`, unless it repeats the last.
fn push_step(steps: &mut Vec<Step>, row: usize, x: u32, value: u32) {
    if steps[row..].last().is_none_or(|&(_, last)| last != value) {
        steps.push((x, value));
    }
}

/// The vertical pass, with its scratch kept from one row to the next.
struct Down {
    area: u32,
    edges: Vec<u32>,
}

impl Down {
    fn new(radius: u32) -> Self {
        Self {
            area: (2 * radius + 1).pow(2),
            edges: Vec::new(),
        }
    }

    /// Adds a window of row sums column by column and writes the average from `row_start`.
    fn emit<'a>(
        &mut self,
        window: impl Iterator<Item = &'a [Step]>,
        width: u32,
        row_start: u32,
        out: &mut RunsBuilder,
    ) {
        let mut readers: Vec<Reader<'a>> = window.map(|row| Reader::new(row, width)).collect();
        self.edges.clear();
        self.edges.extend(
            readers
                .iter()
                .flat_map(|row| row.steps.iter().map(|&(x, _)| x)),
        );
        self.edges.sort_unstable();
        self.edges.dedup();
        for (i, &x) in self.edges.iter().enumerate() {
            let end = self.edges.get(i + 1).copied().unwrap_or(width);
            let sum: u32 = readers.iter_mut().map(|row| row.at(i64::from(x))).sum();
            out.pad_to(row_start + x);
            out.push(end - x, ((sum + self.area / 2) / self.area) as u8);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{LayerMask, LayerRuns};

    fn layer(width: u32, pixels: &[u8]) -> LayerRuns {
        let mut mask = LayerMask::new(width, pixels.len() as u32 / width);
        mask.pixels_mut().copy_from_slice(pixels);
        LayerRuns::from_mask(&mask)
    }

    /// The same box filter over a dense bitmap, reading dark past the panel.
    fn dense_box(width: u32, pixels: &[u8], radius: i64) -> Vec<u8> {
        let height = i64::try_from(pixels.len()).expect("a test panel is small") / i64::from(width);
        let at = |x: i64, y: i64| {
            if x < 0 || y < 0 || x >= i64::from(width) || y >= height {
                0
            } else {
                u32::from(pixels[(y * i64::from(width) + x) as usize])
            }
        };
        let area = ((2 * radius + 1) * (2 * radius + 1)) as u32;
        (0..height)
            .flat_map(|y| (0..i64::from(width)).map(move |x| (x, y)))
            .map(|(x, y)| {
                let sum: u32 = (-radius..=radius)
                    .flat_map(|dy| (-radius..=radius).map(move |dx| at(x + dx, y + dy)))
                    .sum();
                ((sum + area / 2) / area) as u8
            })
            .collect()
    }

    #[test]
    fn an_edge_fades_over_three_pixels_at_radius_one() {
        // A lit half-plane, eight rows tall so the middle rows see no top or bottom.
        let row = [0, 0, 0, 255, 255, 255];
        let pixels: Vec<u8> = row.iter().copied().cycle().take(6 * 8).collect();
        let blurred = layer(6, &pixels).blurred(1).to_mask();

        let middle = &blurred.pixels()[4 * 6..5 * 6];
        assert_eq!(middle, &[0, 0, 85, 170, 255, 170], "a third and two thirds");
    }

    #[test]
    fn a_row_edge_fades_down_the_columns_as_well() {
        let mut pixels = vec![0u8; 5 * 6];
        pixels[3 * 5..].fill(255);
        let blurred = layer(5, &pixels).blurred(1).to_mask();

        let column: Vec<u8> = (0..6).map(|y| blurred.pixels()[y * 5 + 2]).collect();
        assert_eq!(column, vec![0, 0, 85, 170, 255, 170]);
    }

    #[test]
    fn the_runs_agree_with_a_dense_box_filter() {
        let width = 9;
        let pixels: Vec<u8> = (0..width * 7)
            .map(|i| match (i % width, i / width) {
                (2..=6, 2..=4) => 255,
                (7, 1) => 128,
                (0, 6) => 200,
                _ => 0,
            })
            .collect();
        for radius in 1..=3 {
            assert_eq!(
                layer(width, &pixels).blurred(radius).to_mask().pixels(),
                dense_box(width, &pixels, i64::from(radius)).as_slice(),
                "radius {radius}"
            );
        }
    }

    #[test]
    fn radius_zero_and_a_blank_layer_are_left_alone() {
        let lit = layer(3, &[0, 255, 0, 0, 255, 0]);
        assert_eq!(lit.blurred(0), lit);
        let blank = LayerRuns::builder(40, 40).finish();
        assert_eq!(blank.blurred(2), blank);
    }

    #[test]
    fn a_long_edge_costs_a_handful_of_runs() {
        let mut builder = LayerRuns::builder(1000, 5);
        builder.pad_to(2 * 1000 + 100);
        builder.push(800, 255);
        let blurred = builder.finish().blurred(1);
        assert!(
            blurred.runs().len() < 30,
            "a row adds runs for its ends, not per pixel: {:?}",
            blurred.runs()
        );
    }
}
