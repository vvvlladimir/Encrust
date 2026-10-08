use std::f64::consts::PI;
use std::ops::Range;

use core_raster::{LayerRuns, PixelPitch};

/// What one layer cures, read off the runs written for it.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LayerMeasure {
    /// Cured area, square millimetres; a grey pixel counts for the share of light it gets.
    pub area_mm2: f32,
    /// How hard the layer pulls on the film, mm^4: the sum of its pieces' torsion
    /// constants, which Stefan's law makes the suction of. See `docs/design/analysis.md`.
    pub peel_mm4: f32,
    /// Separate pieces the layer cures.
    pub pieces: u32,
}

/// One connected piece of a layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Piece {
    pub area_mm2: f32,
    /// Centre of the cured area, panel millimetres from the top-left pixel.
    pub centre_mm: [f32; 2],
    pub peel_mm4: f32,
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
        let (area, peel) = self.pieces.iter().fold((0.0, 0.0), |(area, peel), piece| {
            (area + piece.area_mm2, peel + piece.peel_mm4)
        });
        LayerMeasure {
            area_mm2: area,
            peel_mm4: peel,
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

    /// This layer in two: the pieces `dropped` flags, and everything else. Both keep the
    /// panel they were cured on, so either can be read against another layer.
    pub(crate) fn split(self, dropped: &[bool]) -> (Self, Self) {
        let Self {
            width,
            height,
            pitch,
            spans,
            pieces,
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
                spans: kept_spans,
                pieces: kept,
            },
            Self {
                width,
                height,
                pitch,
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

/// The diameter of the disc that pulls as hard as `peel_mm4`, which is how a pull is
/// worth stating to someone holding a print.
pub fn equivalent_disc_mm(peel_mm4: f32) -> f32 {
    2.0 * (2.0 * peel_mm4 / std::f32::consts::PI).powf(0.25)
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
            peel_mm4: self.torsion(polar) as f32,
            gyration_mm: (polar / self.area).sqrt() as f32,
        }
    }

    /// Second moment of the area about its own centre, mm^4.
    fn polar(&self) -> f64 {
        let polar = self.xx + self.yy - (self.x * self.x + self.y * self.y) / self.area;
        polar.max(0.0)
    }

    /// Saint-Venant's torsion constant, `A^4 / (4 pi^2 I_p)`, which is exact for a disc.
    fn torsion(&self, polar: f64) -> f64 {
        if polar <= 0.0 {
            return 0.0;
        }
        self.area.powi(4) / (4.0 * PI * PI * polar)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub const UNIT: PixelPitch = PixelPitch { x: 1.0, y: 1.0 };

    pub fn paint(width: u32, height: u32, grey: impl Fn(u32, u32) -> u8) -> LayerRuns {
        let mut builder = LayerRuns::builder(width, height);
        for y in 0..height {
            for x in 0..width {
                builder.push(1, grey(x, y));
            }
        }
        builder.finish()
    }

    fn rectangle(width: u32, height: u32) -> LayerRuns {
        paint(width + 2, height + 2, |x, y| {
            if (1..=width).contains(&x) && (1..=height).contains(&y) {
                255
            } else {
                0
            }
        })
    }

    #[test]
    fn a_dark_layer_measures_nothing() {
        assert_eq!(
            measure(&paint(8, 8, |_, _| 0), UNIT),
            LayerMeasure::default()
        );
    }

    #[test]
    fn a_disc_pulls_as_its_radius_to_the_fourth() {
        // 200 px at 0.05 mm is a disc of 10 mm; Stefan's disc pulls as pi R^4 / 2.
        let pitch = PixelPitch { x: 0.05, y: 0.05 };
        let disc = paint(400, 400, |x, y| {
            let (dx, dy) = (f64::from(x) + 0.5 - 200.0, f64::from(y) + 0.5 - 200.0);
            if dx * dx + dy * dy <= 200.0 * 200.0 {
                255
            } else {
                0
            }
        });
        let measured = measure(&disc, pitch);
        let (area, peel) = (PI * 100.0, PI * 10_000.0 / 2.0);
        assert!(
            (f64::from(measured.area_mm2) - area).abs() / area < 0.01,
            "area {} against pi R^2 = {area}",
            measured.area_mm2
        );
        assert!(
            (f64::from(measured.peel_mm4) - peel).abs() / peel < 0.02,
            "pull {} against pi R^4 / 2 = {peel}",
            measured.peel_mm4
        );
        assert!((equivalent_disc_mm(measured.peel_mm4) - 20.0).abs() < 0.1);
    }

    #[test]
    fn a_strip_pulls_far_less_than_a_square_of_its_area() {
        let square = measure(&rectangle(40, 40), UNIT);
        let strip = measure(&rectangle(400, 4), UNIT);
        assert!((square.area_mm2 - strip.area_mm2).abs() < 1e-3);
        // Torsion constants: 0.141 s^4 for the square, a b^3 / 3 for the strip.
        assert!(
            strip.peel_mm4 * 10.0 < square.peel_mm4,
            "strip {} against square {}",
            strip.peel_mm4,
            square.peel_mm4
        );
    }

    #[test]
    fn two_pieces_pull_less_than_one_of_their_joint_area() {
        let apart = paint(50, 22, |x, y| {
            u8::from((1..21).contains(&y) && ((1..21).contains(&x) || (29..49).contains(&x))) * 255
        });
        let two = measure(&apart, UNIT);
        let one = measure(&rectangle(20, 20), UNIT);
        assert_eq!(two.pieces, 2);
        assert!((two.peel_mm4 - 2.0 * one.peel_mm4).abs() / one.peel_mm4 < 1e-4);

        let joined = measure(&rectangle(40, 20), UNIT);
        assert_eq!(joined.pieces, 1);
        assert!(two.peel_mm4 < joined.peel_mm4);
    }

    #[test]
    fn a_run_across_the_end_of_a_row_is_two_pieces() {
        // The last pixel of the first row and the first of the second are one run in
        // memory and opposite edges of the panel.
        let layer = paint(4, 2, |x, y| {
            u8::from((x, y) == (3, 0) || (x, y) == (0, 1)) * 255
        });
        assert_eq!(layer.runs().iter().filter(|run| run.value > 0).count(), 1);
        assert_eq!(measure(&layer, UNIT).pieces, 2);
    }

    #[test]
    fn neighbouring_greys_along_a_row_are_one_piece() {
        let layer = paint(6, 1, |x, _| [0, 255, 128, 255, 64, 0][x as usize]);
        assert_eq!(measure(&layer, UNIT).pieces, 1);
    }

    #[test]
    fn pieces_touching_only_at_a_corner_are_apart() {
        let layer = paint(2, 2, |x, y| u8::from(x == y) * 255);
        assert_eq!(measure(&layer, UNIT).pieces, 2);
    }

    #[test]
    fn a_half_grey_pixel_cures_half_its_area() {
        let pitch = PixelPitch { x: 0.02, y: 0.03 };
        let layer = paint(10, 10, |_, _| 51);
        let expected = 100.0 * 0.02 * 0.03 * 0.2;
        assert!((measure(&layer, pitch).area_mm2 - expected).abs() < 1e-6);
    }

    #[test]
    fn a_piece_is_centred_on_its_pixels() {
        let cured = cure(&rectangle(4, 2), PixelPitch { x: 0.5, y: 1.0 });
        let [x, y] = cured.pieces()[0].centre_mm;
        // Columns 1..5 at half a millimetre, rows 1..3 at one.
        assert!((x - 1.5).abs() < 1e-6 && (y - 2.0).abs() < 1e-6, "{x}, {y}");
    }

    #[test]
    fn erasing_darkens_the_stretches_and_keeps_every_other_grey() {
        let layer = paint(5, 2, |x, y| if y == 0 { 100 + x as u8 } else { 255 });
        let cuts = [
            Stretch {
                row: 0,
                x0: 1,
                x1: 3,
            },
            Stretch {
                row: 1,
                x0: 4,
                x1: 5,
            },
        ];
        let erased = erase(&layer, &cuts).to_mask().pixels().to_vec();
        assert_eq!(erased, vec![100, 0, 0, 103, 104, 255, 255, 255, 255, 0]);
    }

    #[test]
    fn a_piece_taken_out_leaves_the_rest_renumbered() {
        let layer = paint(6, 1, |x, _| u8::from(x == 0 || x >= 3) * 255);
        let (kept, taken) = cure(&layer, UNIT).split(&[true, false]);
        assert_eq!(
            taken.stretches(),
            vec![Stretch {
                row: 0,
                x0: 0,
                x1: 1
            }]
        );
        assert_eq!(taken.pieces().len(), 1);
        assert_eq!(kept.pieces().len(), 1);
        assert!(kept.spans().iter().all(|span| span.piece == 0));
    }

    #[test]
    fn a_disc_gyrates_about_its_centre_as_its_radius_over_root_two() {
        // A disc of radius R has I_p = pi R^4 / 2 over A = pi R^2, so R / sqrt(2).
        let pitch = PixelPitch { x: 0.05, y: 0.05 };
        let disc = paint(400, 400, |x, y| {
            let (dx, dy) = (f64::from(x) + 0.5 - 200.0, f64::from(y) + 0.5 - 200.0);
            u8::from(dx * dx + dy * dy <= 200.0 * 200.0) * 255
        });
        let gyration = cure(&disc, pitch).pieces()[0].gyration_mm;
        let expected = 10.0 / std::f32::consts::SQRT_2;
        assert!(
            (gyration - expected).abs() / expected < 0.01,
            "{gyration} against R / sqrt(2) = {expected}"
        );
    }

    #[test]
    fn the_runs_of_a_piece_are_its_own_pixels() {
        let layer = paint(6, 2, |x, _| u8::from(x == 0 || x >= 3) * 200);
        let cured = cure(&layer, UNIT);
        let second = cured.runs_of(&[false, true]);
        let lit: Vec<bool> = second.to_mask().pixels().iter().map(|p| *p > 0).collect();
        let expected: Vec<bool> = (0..12).map(|i| i % 6 >= 3).collect();
        assert_eq!(lit, expected);
    }
}
