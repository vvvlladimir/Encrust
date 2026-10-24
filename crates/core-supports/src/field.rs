use core_geometry::{Scalar, Vec2};
use core_raster::{Grey, PixelPitch, RasterSettings, Rasterizer, ScanlineRasterizer, Shading};
use core_slicer::{Contour, Layer};

mod parts;

pub use parts::Parts;

/// Unexposed, in the greyscale a binary raster produces. Everything else is material.
const DARK: u8 = 0;

/// Cells kept clear around the model on every side, so that growing a region never runs
/// off the edge of the grid.
const MARGIN_CELLS: i32 = 4;

/// How far a point may be from the outline it is dropped out of, in cells: half a cell cannot
/// move the field's edge by a whole cell, its finest resolution.
const SIMPLIFY_TOLERANCE_CELLS: Scalar = 0.5;

/// Row counts are held as `i32` so that a row index and a cell index are the same kind
/// of number. A grid that large cannot be built out of a build plate in the first place.
fn rows_of(count: usize) -> i32 {
    i32::try_from(count).unwrap_or(i32::MAX)
}

/// A half-open stretch of cells `[x0, x1)` on one row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub x0: i32,
    pub x1: i32,
}

impl Span {
    pub(crate) fn len(self) -> i32 {
        self.x1 - self.x0
    }

    /// Whether the two touch, corner to corner included: what makes a diagonal chain of
    /// cells one piece rather than several.
    fn adjacent(self, other: Self) -> bool {
        self.x0 <= other.x1 && other.x0 <= self.x1
    }
}

/// The grid a layer is read on: square cells anchored to whole multiples of the cell size from
/// the plate origin, so a model moved a whole number of cells reads the same.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grid {
    pitch_mm: Scalar,
    /// Cell the grid's own (0, 0) sits at, counted from the plate's origin.
    origin: [i32; 2],
    width: u32,
    height: u32,
}

impl Grid {
    /// A grid of `pitch_mm` cells covering `min`..`max` millimetres, with a margin.
    pub fn covering(min: Vec2, max: Vec2, pitch_mm: Scalar) -> Self {
        let first_x = (min.x / pitch_mm).floor() as i32 - MARGIN_CELLS;
        let first_y = (min.y / pitch_mm).floor() as i32 - MARGIN_CELLS;
        let last_x = (max.x / pitch_mm).ceil() as i32 + MARGIN_CELLS;
        let last_y = (max.y / pitch_mm).ceil() as i32 + MARGIN_CELLS;
        Self {
            pitch_mm,
            origin: [first_x, first_y],
            width: (last_x - first_x).max(1) as u32,
            height: (last_y - first_y).max(1) as u32,
        }
    }

    /// Cells the grid covers across and up, in the plate's own cell numbering.
    pub fn columns(&self) -> std::ops::Range<i32> {
        self.origin[0]..self.origin[0] + self.width.cast_signed()
    }

    pub fn rows(&self) -> std::ops::Range<i32> {
        self.origin[1]..self.origin[1] + self.height.cast_signed()
    }

    /// Cells a distance in millimetres spans, rounded up: growing by a whole cell too
    /// many is safer than by one too few.
    pub fn cells_of(&self, distance_mm: Scalar) -> i32 {
        (distance_mm / self.pitch_mm).ceil().max(0.0) as i32
    }

    /// Millimetres a count of cells covers.
    pub fn millimetres_of(&self, cells: i32) -> Scalar {
        cells as Scalar * self.pitch_mm
    }

    /// The middle of a cell, in plate millimetres.
    pub fn centre_mm(&self, x: i32, y: i32) -> Vec2 {
        Vec2::new(
            (x as Scalar + 0.5) * self.pitch_mm,
            (y as Scalar + 0.5) * self.pitch_mm,
        )
    }

    /// The cell a point falls in.
    pub fn cell_of(&self, point: Vec2) -> [i32; 2] {
        [
            (point.x / self.pitch_mm).floor() as i32,
            (point.y / self.pitch_mm).floor() as i32,
        ]
    }

    fn settings(&self) -> RasterSettings {
        RasterSettings {
            width_px: self.width,
            height_px: self.height,
            pitch: PixelPitch {
                x: self.pitch_mm,
                y: self.pitch_mm,
            },
            mirror_x: false,
            mirror_y: false,
            // A cell is lit when the layer covers its middle. Whether a tenth of a
            // millimetre of resin is there is not a question worth a grey level.
            shading: Shading::Binary,
            grey: Grey::default(),
            blur_px: 0,
        }
    }
}

/// An area of one layer as run-length spans of cells, bottom row first, `starts` saying where
/// each row begins; one buffer, not a vector per row (ADR 0020, 0032).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Field {
    /// Row the first entry of `starts` stands for, counted from the plate's origin.
    y0: i32,
    /// One entry per row plus one: row `index` holds `spans[starts[index]..starts[index + 1]]`.
    /// Empty while the field is.
    starts: Vec<u32>,
    spans: Vec<Span>,
}

impl Field {
    /// Reads one sliced layer onto `grid`, moved to the grid origin and thinned to what a cell can
    /// tell apart, since each outline point would be a sweep edge (ADR 0032).
    pub fn of_layer(layer: &Layer, grid: &Grid) -> Self {
        let shift = Vec2::new(
            grid.origin[0] as Scalar * grid.pitch_mm,
            grid.origin[1] as Scalar * grid.pitch_mm,
        );
        let tolerance_mm = grid.pitch_mm * SIMPLIFY_TOLERANCE_CELLS;
        let moved = Layer {
            z: layer.z,
            contours: layer
                .contours
                .iter()
                .map(|contour| {
                    Contour::new(
                        simplified(&contour.points, shift, tolerance_mm),
                        contour.winding,
                    )
                })
                .collect(),
            extra: Vec::new(),
        };

        let settings = grid.settings();
        let Ok(rastered) = ScanlineRasterizer.rasterize(&moved, &settings) else {
            return Self::default();
        };
        Self::of_runs(&rastered, grid)
    }

    /// Everything on the grid that is lit, unpacked from the rasteriser's row-major runs; runs are
    /// cut at row boundaries only, and no two lit runs of a row touch, so no welding.
    fn of_runs(rastered: &core_raster::Rastered, grid: &Grid) -> Self {
        let width = i64::from(grid.width);
        let mut found: Vec<(i32, Span)> = Vec::new();

        let mut at: i64 = 0;
        for run in rastered.runs.runs() {
            let end = at + i64::from(run.length);
            if run.value == DARK {
                at = end;
                continue;
            }
            let mut cursor = at;
            while cursor < end {
                let row = cursor / width;
                let stop = end.min((row + 1) * width);
                if let Ok(up) = i32::try_from(row) {
                    found.push((
                        up + grid.origin[1],
                        Span {
                            x0: (cursor - row * width) as i32 + grid.origin[0],
                            x1: (stop - row * width) as i32 + grid.origin[0],
                        },
                    ));
                }
                cursor = stop;
            }
            at = end;
        }

        let mut rows = Rows::default();
        let mut row: Vec<Span> = Vec::new();
        let mut y = found.first().map_or(0, |&(y, _)| y);
        for (at, span) in found {
            if at != y {
                rows.push(y, &row);
                row.clear();
                y = at;
            }
            row.push(span);
        }
        rows.push(y, &row);
        rows.finish()
    }

    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    /// Rows this field holds anything on, bottom first, with their own index.
    pub fn rows(&self) -> impl Iterator<Item = (i32, &[Span])> {
        self.starts
            .windows(2)
            .enumerate()
            .map(move |(index, edges)| {
                (
                    self.y0 + rows_of(index),
                    &self.spans[edges[0] as usize..edges[1] as usize],
                )
            })
    }

    fn row(&self, y: i32) -> &[Span] {
        let Ok(index) = usize::try_from(y - self.y0) else {
            return &[];
        };
        match (self.starts.get(index), self.starts.get(index + 1)) {
            (Some(&from), Some(&to)) => &self.spans[from as usize..to as usize],
            _ => &[],
        }
    }

    /// Cells this field covers.
    pub fn cell_count(&self) -> u64 {
        self.spans.iter().map(|span| span.len().max(0) as u64).sum()
    }

    /// Area this field covers on `grid`, square millimetres. Only the tests ask.
    #[cfg(test)]
    pub fn area_mm2(&self, grid: &Grid) -> Scalar {
        self.cell_count() as Scalar * grid.pitch_mm * grid.pitch_mm
    }

    /// Whether a cell is covered.
    pub fn contains(&self, x: i32, y: i32) -> bool {
        self.row(y).iter().any(|span| span.x0 <= x && x < span.x1)
    }

    /// The lowest and highest row, and the leftmost and rightmost cell.
    pub fn bounds(&self) -> Option<([i32; 2], [i32; 2])> {
        if self.is_empty() {
            return None;
        }
        let low = self.spans.iter().map(|span| span.x0).min()?;
        let high = self.spans.iter().map(|span| span.x1).max()? - 1;
        let last = self.y0 + rows_of(self.starts.len().saturating_sub(2));
        Some(([low, self.y0], [high, last]))
    }

    /// What this field covers and `other` does not.
    pub fn without(&self, other: &Self) -> Self {
        let mut rows = Rows::default();
        let mut left = Vec::new();
        for (y, spans) in self.rows() {
            left.clear();
            subtract_widened(spans, other.row(y), 0, &mut left);
            rows.push(y, &left);
        }
        rows.finish()
    }

    /// Whether the two share a single cell; only tests ask, placement uses [`Field::adjoins`].
    #[cfg(test)]
    pub fn touches(&self, other: &Self) -> bool {
        self.rows().any(|(y, spans)| overlaps(spans, other.row(y)))
    }

    /// Whether any cell of this field sits next to a cell of `other`, corners included.
    pub fn adjoins(&self, other: &Self) -> bool {
        self.rows()
            .any(|(y, spans)| (-1..=1).any(|dy| neighbouring(spans, other.row(y + dy))))
    }

    /// What this field covers that `other` does not reach within `radius` cells, equal to
    /// `self.without(&other.grown(radius))` without building the grown field; the hottest sweep.
    pub fn uncarried_by(&self, other: &Self, radius: i32) -> Self {
        if radius <= 0 {
            return self.without(other);
        }

        let reach = disc_reach(radius);
        let mut rows = Rows::default();
        let (mut left, mut kept) = (Vec::new(), Vec::new());
        for (y, spans) in self.rows() {
            left.clear();
            subtract_widened(spans, other.row(y), reach[0], &mut left);
            for dy in 1..=radius {
                let widen = reach[dy as usize];
                for step in [dy, -dy] {
                    if left.is_empty() {
                        break;
                    }
                    kept.clear();
                    subtract_widened(&left, other.row(y + step), widen, &mut kept);
                    std::mem::swap(&mut left, &mut kept);
                }
            }
            rows.push(y, &left);
        }
        rows.finish()
    }

    /// Whether anything of this field lies within a disc of `radius` cells round `(x, y)`: one cell
    /// of [`Field::uncarried_by`] asked alone.
    pub fn reaches(&self, x: i32, y: i32, radius: i32) -> bool {
        let radius = radius.max(0);
        (-radius..=radius).any(|dy| {
            let widen = ((radius * radius - dy * dy) as Scalar).sqrt().floor() as i32;
            self.row(y + dy)
                .iter()
                .any(|span| span.x0 - widen <= x && x < span.x1 + widen)
        })
    }

    /// This field grown by `radius` cells as a disc, not a square, whose corner overreaches along
    /// a diagonal; tests only, placement uses [`Field::uncarried_by`].
    #[cfg(test)]
    pub fn grown(&self, radius: i32) -> Self {
        if radius <= 0 || self.is_empty() {
            return self.clone();
        }

        let reach = disc_reach(radius);
        let first = self.y0 - radius;
        let last = self.y0 + rows_of(self.starts.len()) + radius;
        let mut rows = Rows::default();
        let mut row: Vec<Span> = Vec::new();
        for y in first..=last {
            row.clear();
            for dy in -radius..=radius {
                let widen = reach[dy.unsigned_abs() as usize];
                row.extend(self.row(y + dy).iter().map(|span| Span {
                    x0: span.x0 - widen,
                    x1: span.x1 + widen,
                }));
            }
            merge(&mut row);
            rows.push(y, &row);
        }
        rows.finish()
    }

    /// This field pulled back from its own edge by `radius` cells: what is left after a
    /// disc of that size can no longer be put down inside it.
    pub fn shrunk(&self, radius: i32) -> Self {
        if radius <= 0 || self.is_empty() {
            return self.clone();
        }

        let reach = disc_reach(radius);
        let mut rows = Rows::default();
        let (mut kept, mut narrow, mut both) = (Vec::new(), Vec::new(), Vec::new());
        for (y, spans) in self.rows() {
            kept.clear();
            narrowed(spans, reach[0], &mut kept);
            for dy in 1..=radius {
                let widen = reach[dy as usize];
                for step in [dy, -dy] {
                    if kept.is_empty() {
                        break;
                    }
                    narrow.clear();
                    narrowed(self.row(y + step), widen, &mut narrow);
                    both.clear();
                    intersect(&kept, &narrow, &mut both);
                    std::mem::swap(&mut kept, &mut both);
                }
            }
            rows.push(y, &kept);
        }
        rows.finish()
    }

    /// The outline of this field, one cell thick. Where a part peels off the film first,
    /// and where a lattice of supports leaves the widest gap.
    pub fn rim(&self) -> Self {
        self.without(&self.shrunk(1))
    }

    /// Everything either field covers.
    pub fn with(&self, other: &Self) -> Self {
        if self.is_empty() {
            return other.clone();
        }
        if other.is_empty() {
            return self.clone();
        }

        let first = self.y0.min(other.y0);
        let last =
            (self.y0 + rows_of(self.starts.len())).max(other.y0 + rows_of(other.starts.len()));
        let mut rows = Rows::default();
        let mut row: Vec<Span> = Vec::new();
        for y in first..=last {
            row.clear();
            row.extend_from_slice(self.row(y));
            row.extend_from_slice(other.row(y));
            merge(&mut row);
            rows.push(y, &row);
        }
        rows.finish()
    }

    /// One piece built from its own spans, which arrive row by row from the bottom.
    fn of_spans(spans: Vec<(i32, Span)>) -> Self {
        let mut rows = Rows::default();
        let mut row: Vec<Span> = Vec::new();
        let mut y = spans.first().map_or(0, |&(y, _)| y);
        for (at, span) in spans {
            if at != y {
                rows.push(y, &row);
                row.clear();
                y = at;
            }
            row.push(span);
        }
        rows.push(y, &row);
        rows.finish()
    }
}

/// Collects a field one row at a time, bottom first; empty leading rows are dropped and gaps
/// filled, so equal cells compare equal.
#[derive(Debug, Default)]
struct Rows {
    y0: i32,
    starts: Vec<u32>,
    spans: Vec<Span>,
}

impl Rows {
    /// Adds row `y`, whose spans must be sorted and disjoint. Rows must arrive in
    /// increasing order of `y`.
    fn push(&mut self, y: i32, spans: &[Span]) {
        if spans.is_empty() {
            return;
        }
        if self.starts.is_empty() {
            self.y0 = y;
            self.starts.push(0);
        }

        let next = self.y0 + rows_of(self.starts.len() - 1);
        debug_assert!(y >= next, "rows arrive bottom first");
        for _ in next..y {
            self.starts.push(self.spans.len() as u32);
        }
        self.spans.extend_from_slice(spans);
        self.starts.push(self.spans.len() as u32);
    }

    fn finish(self) -> Field {
        Field {
            y0: self.y0,
            starts: self.starts,
            spans: self.spans,
        }
    }
}

/// One contour moved back by `shift` and thinned to `tolerance_mm` in one corridor pass.
/// The walk starts at the lowest point so layers of one wall thin to the same outline.
fn simplified(points: &[Vec2], shift: Vec2, tolerance_mm: Scalar) -> Vec<Vec2> {
    if points.len() < 4 {
        return points.iter().map(|point| *point - shift).collect();
    }

    let first = lowest_point(points);
    let ring = points[first..].iter().chain(&points[..first]);
    let mut kept: Vec<Vec2> = Vec::with_capacity(points.len());
    let mut anchor = points[first];
    let mut along = Vec2::ZERO;
    let mut previous = anchor;
    kept.push(anchor - shift);
    for point in ring.skip(1) {
        if along == Vec2::ZERO {
            along = (*point - anchor).normalize_or_zero();
            previous = *point;
            continue;
        }
        // Distance from the corridor's centre line, which is the cross product in 2D.
        let offset = *point - anchor;
        if offset.x.mul_add(along.y, -(offset.y * along.x)).abs() <= tolerance_mm {
            previous = *point;
            continue;
        }
        // The run ends at the last point that was still inside the corridor, so a corner
        // is kept rather than cut off.
        kept.push(previous - shift);
        anchor = previous;
        along = (*point - anchor).normalize_or_zero();
        previous = *point;
    }

    // The last point of the ring closes the outline back onto the first, so it is kept
    // whatever the corridor says: the closing edge is not part of any run.
    let last = points[(first + points.len() - 1) % points.len()] - shift;
    if kept.last() != Some(&last) {
        kept.push(last);
    }
    if kept.len() < 3 {
        return points.iter().map(|point| *point - shift).collect();
    }
    kept
}

/// Where the ring's lowest point sits, lowest by y and then by x. Any rotation of the
/// same ring answers with the same point.
fn lowest_point(points: &[Vec2]) -> usize {
    let mut at = 0;
    for (index, point) in points.iter().enumerate().skip(1) {
        let best = points[at];
        if (point.y, point.x) < (best.y, best.x) {
            at = index;
        }
    }
    at
}

/// Sorts a row's spans and welds the ones that touch, so every row stays a disjoint list
/// in increasing order. Only the tests build a row out of order.
fn merge(spans: &mut Vec<Span>) {
    if spans.len() > 1 {
        spans.sort_unstable_by_key(|span| span.x0);
    }
    let mut kept = 0;
    for index in 1..spans.len() {
        let span = spans[index];
        if span.x0 <= spans[kept].x1 {
            spans[kept].x1 = spans[kept].x1.max(span.x1);
        } else {
            kept += 1;
            spans[kept] = span;
        }
    }
    spans.truncate(spans.len().min(kept + 1));
}

/// Writes into `left` the parts of `spans` that `cut` leaves once its spans are pushed out by
/// `widen` cells; applied as read, since the widened row is discarded at once.
fn subtract_widened(spans: &[Span], cut: &[Span], widen: i32, left: &mut Vec<Span>) {
    if cut.is_empty() {
        left.extend_from_slice(spans);
        return;
    }
    for span in spans {
        let mut x = span.x0;
        for hole in cut {
            let (low, high) = (hole.x0 - widen, hole.x1 + widen);
            if high <= x {
                continue;
            }
            if low >= span.x1 {
                break;
            }
            if low > x {
                left.push(Span { x0: x, x1: low });
            }
            x = x.max(high);
            if x >= span.x1 {
                break;
            }
        }
        if x < span.x1 {
            left.push(Span { x0: x, x1: span.x1 });
        }
    }
}

/// How far a disc of `radius` cells reaches sideways on each of its own rows.
fn disc_reach(radius: i32) -> Vec<i32> {
    (0..=radius)
        .map(|dy| ((radius * radius - dy * dy) as Scalar).sqrt().floor() as i32)
        .collect()
}

/// Every span pulled in by `width` cells on both sides, dropping what is left of the ones
/// too narrow to survive it.
fn narrowed(spans: &[Span], width: i32, out: &mut Vec<Span>) {
    out.extend(spans.iter().filter_map(|span| {
        let narrow = Span {
            x0: span.x0 + width,
            x1: span.x1 - width,
        };
        (narrow.x0 < narrow.x1).then_some(narrow)
    }));
}

/// The parts both rows cover.
fn intersect(spans: &[Span], other: &[Span], both: &mut Vec<Span>) {
    let (mut a, mut b) = (0, 0);
    while a < spans.len() && b < other.len() {
        let overlap = Span {
            x0: spans[a].x0.max(other[b].x0),
            x1: spans[a].x1.min(other[b].x1),
        };
        if overlap.x0 < overlap.x1 {
            both.push(overlap);
        }
        if spans[a].x1 < other[b].x1 {
            a += 1;
        } else {
            b += 1;
        }
    }
}

/// Whether any two spans of the rows touch or overlap, ends included.
fn neighbouring(spans: &[Span], other: &[Span]) -> bool {
    let (mut a, mut b) = (0, 0);
    while a < spans.len() && b < other.len() {
        if spans[a].adjacent(other[b]) {
            return true;
        }
        if spans[a].x1 < other[b].x1 {
            a += 1;
        } else {
            b += 1;
        }
    }
    false
}

#[cfg(test)]
fn overlaps(spans: &[Span], other: &[Span]) -> bool {
    let (mut a, mut b) = (0, 0);
    while a < spans.len() && b < other.len() {
        if spans[a].x1 <= other[b].x0 {
            a += 1;
        } else if other[b].x1 <= spans[a].x0 {
            b += 1;
        } else {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests;
