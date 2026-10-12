use crate::layer::Cured;

/// A sealed hole narrower than this at its widest pulls too little to name, mm^2: a disc
/// some 5.6 mm across, which a full vacuum holds with about 2.5 N.
pub const MIN_CUP_MM2: f32 = 25.0;
/// A sealed hole holding less resin than this is too shallow to name, mm^3: 0.02 ml, the
/// pocket under a letter's counter or a pin's socket.
pub const MIN_CUP_MM3: f32 = 20.0;

/// A run of a row's dark pixels between two lit ones, and the hole it is part of.
#[derive(Debug, Clone, Copy)]
struct Gap {
    row: u32,
    x0: u32,
    x1: u32,
    hole: u32,
}

/// Dark pixels closed in on every side by what a layer cures, as one region.
#[derive(Debug, Clone, Copy, Default)]
struct Hole {
    /// Whether air reaches it from below, through the layers already printed.
    vented: bool,
    cup: Option<u32>,
    pixels: u64,
    x_px: f64,
    y_px: f64,
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
        let (gaps, mut holes) = holes(layer);
        for gap in &gaps {
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

/// The dark runs of `layer` between two lit ones, grouped into holes, with the regions
/// that reach the panel's edge left out: those are open air.
fn holes(layer: &Cured) -> (Vec<Gap>, Vec<Hole>) {
    let spans = layer.spans();
    let mut gaps: Vec<Gap> = Vec::new();
    let mut start = 0;
    while start < spans.len() {
        let row = spans[start].row;
        let mut end = start;
        while end + 1 < spans.len() && spans[end + 1].row == row {
            if spans[end + 1].x0 > spans[end].x1 {
                gaps.push(Gap {
                    row,
                    x0: spans[end].x1,
                    x1: spans[end + 1].x0,
                    hole: 0,
                });
            }
            end += 1;
        }
        start = end + 1;
    }

    let outside = gaps.len();
    let mut parent: Vec<usize> = (0..=outside).collect();
    for (index, gap) in gaps.iter().enumerate() {
        for next in [gap.row.checked_sub(1), gap.row.checked_add(1)] {
            let lit = next
                .filter(|next| *next < layer.height())
                .map(|next| row_range(spans, next, |span| span.row))
                .filter(|range| !range.is_empty());
            let Some(lit) = lit else {
                join(&mut parent, index, outside);
                continue;
            };
            // Past the row's first or last lit pixel is open air.
            if gap.x0 < spans[lit.start].x0 || gap.x1 > spans[lit.end - 1].x1 {
                join(&mut parent, index, outside);
            }
            let row = spans[lit.start].row;
            for other in row_range(&gaps, row, |other| other.row) {
                if gaps[other].x0 < gap.x1 && gap.x0 < gaps[other].x1 {
                    join(&mut parent, index, other);
                }
            }
        }
    }

    let outside_root = find(&mut parent, outside);
    let mut numbered = vec![u32::MAX; outside + 1];
    let mut holes: Vec<Hole> = Vec::new();
    let mut kept = Vec::with_capacity(gaps.len());
    for (index, gap) in gaps.iter().enumerate() {
        let root = find(&mut parent, index);
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
    (kept, holes)
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
