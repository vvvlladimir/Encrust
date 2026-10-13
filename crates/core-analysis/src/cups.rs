use crate::layer::{Cured, Span};

/// A sealed hole narrower than this at its widest pulls too little to name, mm^2: a disc
/// some 5.6 mm across, which a full vacuum holds with about 2.5 N.
pub const MIN_CUP_MM2: f32 = 25.0;
/// A sealed hole holding less resin than this is too shallow to name, mm^3: 0.02 ml, the
/// pocket under a letter's counter or a pin's socket.
pub const MIN_CUP_MM3: f32 = 20.0;

/// A run of a row's dark pixels between two lit ones, and the hole it is part of.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Gap {
    row: u32,
    x0: u32,
    x1: u32,
    hole: u32,
}

/// Dark pixels closed in on every side by what a layer cures, as one region.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Hole {
    /// Whether air reaches it from below, through the layers already printed.
    vented: bool,
    cup: Option<u32>,
    pixels: u64,
    x_px: f64,
    y_px: f64,
}

/// The holes of one layer, read off that layer alone. That is what lets them be found
/// while layers are cured in parallel, leaving only their linking up the stack in order.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Holes {
    gaps: Vec<Gap>,
    holes: Vec<Hole>,
}

/// One sealed hole, followed up the stack while it stays sealed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Cup {
    pub peak_layer: usize,
    pub peak_mm2: f32,
    /// Where it is widest, panel millimetres from the top-left pixel.
    pub at_mm: [f32; 2],
    pub volume_mm3: f32,
}

/// Holes in each layer that air cannot reach from below: the plate under the first one,
/// or a ceiling the model printed over them. Every lift pulls a vacuum in them. See
/// `docs/design/analysis.md`.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Cups {
    below: Vec<(u32, u32, u32, u32)>,
    below_holes: Vec<(bool, Option<u32>)>,
    traced: Vec<Cup>,
}

impl Cups {
    /// Reads the holes of layer `index`, `thickness_mm` thick, against the layer under it,
    /// or the plate where there is none.
    pub fn push(&mut self, index: usize, layer: &Cured, below: Option<&Cured>, thickness_mm: f32) {
        let Holes { gaps, holes } = layer.holes();
        let mut holes = holes.clone();
        for gap in gaps {
            let hole = &mut holes[gap.hole as usize];
            let (vented, cup) = self.under(gap, below);
            hole.vented |= vented;
            hole.cup = hole.cup.or(cup);
        }
        let pitch = layer.pitch();
        let pixel_mm2 = pitch.x * pitch.y;
        for hole in holes.iter_mut().filter(|hole| !hole.vented) {
            let area_mm2 = hole.pixels as f32 * pixel_mm2;
            let at_mm = [
                (hole.x_px / hole.pixels as f64) as f32 * pitch.x,
                (hole.y_px / hole.pixels as f64) as f32 * pitch.y,
            ];
            let id = *hole.cup.get_or_insert_with(|| {
                self.traced.push(Cup {
                    peak_layer: index,
                    peak_mm2: 0.0,
                    at_mm,
                    volume_mm3: 0.0,
                });
                (self.traced.len() - 1) as u32
            });
            let cup = &mut self.traced[id as usize];
            cup.volume_mm3 += area_mm2 * thickness_mm;
            if area_mm2 > cup.peak_mm2 {
                (cup.peak_layer, cup.peak_mm2, cup.at_mm) = (index, area_mm2, at_mm);
            }
        }
        self.below = gaps
            .iter()
            .map(|gap| (gap.row, gap.x0, gap.x1, gap.hole))
            .collect();
        self.below_holes = holes.iter().map(|hole| (hole.vented, hole.cup)).collect();
    }

    /// Every sealed hole wide and deep enough to name.
    pub fn found(&self) -> impl Iterator<Item = &Cup> {
        self.traced
            .iter()
            .filter(|cup| cup.peak_mm2 >= MIN_CUP_MM2 && cup.volume_mm3 >= MIN_CUP_MM3)
    }

    /// Whether air reaches `gap` from the layer under it, and the sealed cup it carries on
    /// where it does not.
    fn under(&self, gap: &Gap, below: Option<&Cured>) -> (bool, Option<u32>) {
        let Some(below) = below else {
            return (false, None);
        };
        let length = gap.x1 - gap.x0;
        let overlap = |x0: u32, x1: u32| gap.x1.min(x1).saturating_sub(gap.x0.max(x0));
        let spans = below.spans();
        let lit: u32 = spans[row_range(spans, gap.row, |span| span.row)]
            .iter()
            .map(|span| overlap(span.x0, span.x1))
            .sum();
        let (mut covered, mut vented, mut cup) = (lit, false, None);
        for (_, x0, x1, hole) in &self.below[row_range(&self.below, gap.row, |entry| entry.0)] {
            let shared = overlap(*x0, *x1);
            if shared == 0 {
                continue;
            }
            covered += shared;
            let (hole_vented, hole_cup) = self.below_holes[*hole as usize];
            vented |= hole_vented;
            cup = cup.or(hole_cup);
        }
        // Whatever of the gap is neither cured nor a hole below is open air.
        (vented || covered < length, cup)
    }
}

/// The entries of `sorted`, ordered by row, that lie on `row`.
fn row_range<T>(sorted: &[T], row: u32, row_of: impl Fn(&T) -> u32) -> std::ops::Range<usize> {
    let start = sorted.partition_point(|entry| row_of(entry) < row);
    let end = sorted.partition_point(|entry| row_of(entry) <= row);
    start..end
}

impl Holes {
    /// The dark runs between two lit ones of a layer's `spans`, in reading order on a
    /// panel `height` rows tall, grouped into holes, with the regions that reach the
    /// panel's edge left out: those are open air.
    pub(crate) fn of(spans: &[Span], height: u32) -> Self {
        let gaps = gaps_between(spans);
        let rows = height.max(spans.last().map_or(0, |span| span.row + 1));
        let lit = row_starts(spans.iter().map(|span| span.row), rows);
        let dark = row_starts(gaps.iter().map(|gap| gap.row), rows);
        let outside = gaps.len();
        let mut parent: Vec<usize> = (0..=outside).collect();
        for (index, gap) in gaps.iter().enumerate() {
            for next in [gap.row.checked_sub(1), gap.row.checked_add(1)] {
                let Some(next) = next.filter(|next| *next < height) else {
                    join(&mut parent, index, outside);
                    continue;
                };
                let lit = row_of(&lit, next);
                if lit.is_empty() {
                    join(&mut parent, index, outside);
                    continue;
                }
                // Past the row's first or last lit pixel is open air.
                if gap.x0 < spans[lit.start].x0 || gap.x1 > spans[lit.end - 1].x1 {
                    join(&mut parent, index, outside);
                }
                for other in row_of(&dark, next) {
                    if gaps[other].x0 < gap.x1 && gap.x0 < gaps[other].x1 {
                        join(&mut parent, index, other);
                    }
                }
            }
        }
        numbered(&gaps, &mut parent, outside)
    }
}

fn gaps_between(spans: &[Span]) -> Vec<Gap> {
    spans
        .windows(2)
        .filter(|pair| pair[0].row == pair[1].row && pair[1].x0 > pair[0].x1)
        .map(|pair| Gap {
            row: pair[0].row,
            x0: pair[0].x1,
            x1: pair[1].x0,
            hole: 0,
        })
        .collect()
}

/// Where each row's entries start in a list ordered by row, with where the last one ends
/// after them: row `r` is `starts[r]..starts[r + 1]`. One pass, where a search per gap
/// was most of what a plate of many parts spent here.
fn row_starts(rows: impl Iterator<Item = u32>, height: u32) -> Vec<usize> {
    let mut starts = vec![0; height as usize + 1];
    for row in rows {
        starts[row as usize + 1] += 1;
    }
    for row in 1..starts.len() {
        starts[row] += starts[row - 1];
    }
    starts
}

fn row_of(starts: &[usize], row: u32) -> std::ops::Range<usize> {
    starts[row as usize]..starts[row as usize + 1]
}

/// The gaps not joined to the outside, each with the number of the hole it is part of,
/// and what each hole holds.
fn numbered(gaps: &[Gap], parent: &mut [usize], outside: usize) -> Holes {
    let outside_root = find(parent, outside);
    let mut numbered = vec![u32::MAX; outside + 1];
    let mut holes: Vec<Hole> = Vec::new();
    let mut kept = Vec::new();
    for (index, gap) in gaps.iter().enumerate() {
        let root = find(parent, index);
        if root == outside_root {
            continue;
        }
        if numbered[root] == u32::MAX {
            numbered[root] = holes.len() as u32;
            holes.push(Hole::default());
        }
        let hole = &mut holes[numbered[root] as usize];
        let pixels = u64::from(gap.x1 - gap.x0);
        hole.pixels += pixels;
        hole.x_px += f64::from(gap.x0 + gap.x1) / 2.0 * pixels as f64;
        hole.y_px += (f64::from(gap.row) + 0.5) * pixels as f64;
        kept.push(Gap {
            hole: numbered[root],
            ..*gap
        });
    }
    Holes { gaps: kept, holes }
}

fn find(parent: &mut [usize], mut node: usize) -> usize {
    while parent[node] != node {
        parent[node] = parent[parent[node]];
        node = parent[node];
    }
    node
}

fn join(parent: &mut [usize], a: usize, b: usize) {
    let (a, b) = (find(parent, a), find(parent, b));
    if a != b {
        parent[a.max(b)] = a.min(b);
    }
}
