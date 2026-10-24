use std::ops::Range;

use core_raster::{LayerRuns, PixelPitch};

use crate::cups::Holes;
use crate::pull::pull_n;

/// What one layer cures, read off the runs written for it.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LayerMeasure {
    /// Cured area, square millimetres; a grey pixel counts for the share of light it gets.
    pub area_mm2: f32,
    /// How hard the layer pulls on the film, newtons: the sum of its pieces' pulls.
    pub pull_n: f32,
    /// Separate pieces the layer cures.
    pub pieces: u32,
}

/// One connected piece of a layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Piece {
    pub area_mm2: f32,
    /// Centre of the cured area, panel millimetres from the top-left pixel.
    pub centre_mm: [f32; 2],
    /// What the piece pulls on the film, newtons. See `docs/design/analysis.md`.
    pub pull_n: f32,
    /// Radius of gyration of the cured area about its centre, millimetres: how far off
    /// that centre the pull acts while the film still holds half the piece.
    pub gyration_mm: f32,
}

/// A layer's lit pixels grouped into pieces: what the stack is read from.
#[derive(Debug, Clone, PartialEq)]
pub struct Cured {
    width: u32,
    height: u32,
    pitch: PixelPitch,
    spans: Vec<Span>,
    pieces: Vec<Piece>,
    holes: Holes,
}

/// Lit pixels of one row, `x0..x1`, whatever their greys, and the piece they belong to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Span {
    pub row: u32,
    pub x0: u32,
    pub x1: u32,
    pub piece: u32,
}

/// Lit pixels `x0..x1` of one row, taken out of a layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stretch {
    pub row: u32,
    pub x0: u32,
    pub x1: u32,
}

/// Groups one layer on a panel of `pitch` into pieces, in one pass over its runs.
pub fn cure(layer: &LayerRuns, pitch: PixelPitch) -> Cured {
    let raw = spans(layer, pitch);
    let mut roots = Roots::new(raw.len());
    join(&raw, &mut roots);

    let mut piece_of = vec![u32::MAX; raw.len()];
    let mut moments: Vec<Moments> = Vec::new();
    let mut spans = Vec::with_capacity(raw.len());
    for (index, span) in raw.iter().enumerate() {
        let root = roots.root(index);
        if piece_of[root] == u32::MAX {
            piece_of[root] = moments.len() as u32;
            moments.push(Moments::default());
        }
        let piece = piece_of[root];
        moments[piece as usize].merge(&span.moments);
        spans.push(Span {
            row: span.row,
            x0: span.x0,
            x1: span.x1,
            piece,
        });
    }

    Cured {
        width: layer.width(),
        height: layer.height(),
        pitch,
        holes: Holes::of(&spans, layer.height()),
        spans,
        pieces: moments.iter().map(Moments::piece).collect(),
    }
}

/// The area and pull of one layer on a panel of `pitch`.
pub fn measure(layer: &LayerRuns, pitch: PixelPitch) -> LayerMeasure {
    cure(layer, pitch).measure()
}

impl Cured {
    pub fn measure(&self) -> LayerMeasure {
        let (area, pull) = self.pieces.iter().fold((0.0, 0.0), |(area, pull), piece| {
            (area + piece.area_mm2, pull + piece.pull_n)
        });
        LayerMeasure {
            area_mm2: area,
            pull_n: pull,
            pieces: self.pieces.len() as u32,
        }
    }

    pub fn pieces(&self) -> &[Piece] {
        &self.pieces
    }

    pub fn pitch(&self) -> PixelPitch {
        self.pitch
    }

    pub(crate) fn spans(&self) -> &[Span] {
        &self.spans
    }

    pub(crate) fn holes(&self) -> &Holes {
        &self.holes
    }

    /// This layer in two: the pieces `dropped` flags, and everything else. Both keep the
    /// panel they were cured on, so either can be read against another layer.
    pub(crate) fn split(self, dropped: &[bool]) -> (Self, Self) {
        let Self {
            width,
            height,
            pitch,
            spans,
            pieces,
            holes: _,
        } = self;
        let mut renumbered = vec![0u32; pieces.len()];
        let mut halves: [(Vec<Piece>, Vec<Span>); 2] =
            [(Vec::new(), Vec::new()), (Vec::new(), Vec::new())];
        for (index, piece) in pieces.iter().enumerate() {
            let (pieces, _) = &mut halves[usize::from(dropped[index])];
            renumbered[index] = pieces.len() as u32;
            pieces.push(*piece);
        }
        for span in spans {
            let (_, spans) = &mut halves[usize::from(dropped[span.piece as usize])];
            spans.push(Span {
                piece: renumbered[span.piece as usize],
                ..span
            });
        }
        let [(kept, kept_spans), (taken, taken_spans)] = halves;
        (
            Self {
                width,
                height,
                pitch,
                holes: Holes::of(&kept_spans, height),
                spans: kept_spans,
                pieces: kept,
            },
            Self {
                width,
                height,
                pitch,
                holes: Holes::of(&taken_spans, height),
                spans: taken_spans,
                pieces: taken,
            },
        )
    }

    /// The stretches of panel this layer's pixels cover, in reading order.
    pub(crate) fn stretches(&self) -> Vec<Stretch> {
        self.spans
            .iter()
            .map(|span| Stretch {
                row: span.row,
                x0: span.x0,
                x1: span.x1,
            })
            .collect()
    }

    /// The pixels of the pieces `wanted` names, lit, and everything else dark.
    pub(crate) fn runs_of(&self, wanted: &[bool]) -> LayerRuns {
        let mut builder = LayerRuns::builder(self.width, self.height);
        for span in &self.spans {
            if wanted.get(span.piece as usize) == Some(&true) {
                builder.pad_to(span.row * self.width + span.x0);
                builder.push(span.x1 - span.x0, u8::MAX);
            }
        }
        builder.finish()
    }
}

/// `layer` with `stretches`, in reading order, made dark: what a file is written with once
/// its islands are taken out.
pub fn erase(layer: &LayerRuns, stretches: &[Stretch]) -> LayerRuns {
    let width = u64::from(layer.width());
    let mut cuts = stretches
        .iter()
        .map(|cut| {
            let row = u64::from(cut.row) * width;
            (row + u64::from(cut.x0), row + u64::from(cut.x1))
        })
        .peekable();
    let mut builder = LayerRuns::builder(layer.width(), layer.height());
    let mut at = 0u64;
    for run in layer.runs() {
        let end = at + u64::from(run.length);
        let mut pos = at;
        while pos < end {
            while cuts.next_if(|&(_, cut_end)| cut_end <= pos).is_some() {}
            let (length, value) = match cuts.peek() {
                Some(&(start, stop)) if start <= pos => (stop.min(end) - pos, 0),
                Some(&(start, _)) if start < end => (start - pos, run.value),
                _ => (end - pos, run.value),
            };
            builder.push(length as u32, value);
            pos += length;
        }
        at = end;
    }
    builder.finish()
}

/// A span while the layer is being read, with what its pixels add up to.
#[derive(Debug, Clone, Copy)]
struct Raw {
    row: u32,
    x0: u32,
    x1: u32,
    moments: Moments,
}

/// The lit stretches of each row in reading order. An anti-aliased edge is a run a pixel,
/// so runs are merged here rather than joined as pieces later.
fn spans(layer: &LayerRuns, pitch: PixelPitch) -> Vec<Raw> {
    let width = u64::from(layer.width());
    let mut spans: Vec<Raw> = Vec::new();
    let mut at = 0u64;
    for run in layer.runs() {
        let end = at + u64::from(run.length);
        if run.value > 0 && width > 0 {
            let weight = f64::from(run.value) / 255.0;
            let mut start = at;
            while start < end {
                let row = (start / width) as u32;
                let x0 = (start % width) as u32;
                let x1 = (u64::from(x0) + end - start).min(width) as u32;
                match spans.last_mut() {
                    Some(last) if last.row == row && last.x1 == x0 => last.x1 = x1,
                    _ => spans.push(Raw {
                        row,
                        x0,
                        x1,
                        moments: Moments::default(),
                    }),
                }
                if let Some(last) = spans.last_mut() {
                    last.moments.add(row, x0..x1, weight, pitch);
                }
                start += u64::from(x1 - x0);
            }
        }
        at = end;
    }
    spans
}

/// Unites spans overlapping the row above.
fn join(spans: &[Raw], roots: &mut Roots) {
    let mut above = 0..0;
    let mut start = 0;
    while start < spans.len() {
        let row = spans[start].row;
        let end = start + spans[start..].iter().take_while(|s| s.row == row).count();
        if !above.is_empty() && spans[above.start].row + 1 == row {
            overlapping(spans, above, start..end, roots);
        }
        above = start..end;
        start = end;
    }
}

/// Unites every span of `here` with the spans of the row above it overlaps, walking both
/// rows once.
fn overlapping(spans: &[Raw], above: Range<usize>, here: Range<usize>, roots: &mut Roots) {
    let (mut up, mut down) = (above.start, here.start);
    while up < above.end && down < here.end {
        let (a, b) = (&spans[up], &spans[down]);
        if a.x0 < b.x1 && b.x0 < a.x1 {
            roots.union(up, down);
        }
        if a.x1 <= b.x1 {
            up += 1;
        } else {
            down += 1;
        }
    }
}

/// Union-find over span indices.
struct Roots {
    parent: Vec<usize>,
}

impl Roots {
    fn new(count: usize) -> Self {
        Self {
            parent: (0..count).collect(),
        }
    }

    fn root(&mut self, mut index: usize) -> usize {
        while self.parent[index] != index {
            self.parent[index] = self.parent[self.parent[index]];
            index = self.parent[index];
        }
        index
    }

    fn union(&mut self, a: usize, b: usize) {
        let (a, b) = (self.root(a), self.root(b));
        self.parent[a.max(b)] = a.min(b);
    }
}

/// Area and its first and second moments about the panel's corner, mm.
#[derive(Debug, Clone, Copy, Default)]
struct Moments {
    area: f64,
    x: f64,
    y: f64,
    xx: f64,
    yy: f64,
}

impl Moments {
    /// Adds pixels `columns` of `row` as the rectangle of panel they cover, weighted by
    /// their grey.
    fn add(&mut self, row: u32, columns: Range<u32>, weight: f64, pitch: PixelPitch) {
        let (px, py) = (f64::from(pitch.x), f64::from(pitch.y));
        let (x0, x1) = (f64::from(columns.start) * px, f64::from(columns.end) * px);
        let (y0, y1) = (f64::from(row) * py, f64::from(row + 1) * py);
        let (across, down) = ((x1 - x0) * weight, (y1 - y0) * weight);

        self.area += across * (y1 - y0);
        self.x += down * (x1 * x1 - x0 * x0) / 2.0;
        self.xx += down * (x1.powi(3) - x0.powi(3)) / 3.0;
        self.y += across * (y1 * y1 - y0 * y0) / 2.0;
        self.yy += across * (y1.powi(3) - y0.powi(3)) / 3.0;
    }

    fn merge(&mut self, other: &Self) {
        self.area += other.area;
        self.x += other.x;
        self.y += other.y;
        self.xx += other.xx;
        self.yy += other.yy;
    }

    fn piece(&self) -> Piece {
        let polar = self.polar();
        Piece {
            area_mm2: self.area as f32,
            centre_mm: [(self.x / self.area) as f32, (self.y / self.area) as f32],
            pull_n: pull_n(self.area, polar) as f32,
            gyration_mm: (polar / self.area).sqrt() as f32,
        }
    }

    /// Second moment of the area about its own centre, mm^4.
    fn polar(&self) -> f64 {
        let polar = self.xx + self.yy - (self.x * self.x + self.y * self.y) / self.area;
        polar.max(0.0)
    }
}

#[cfg(test)]
pub(crate) mod tests;
