use glam::Vec2;

use crate::runs::RunsBuilder;
use crate::settings::Grey;

/// One non-horizontal contour segment, ready to be crossed by a sample line.
///
/// Horizontal segments are left out: they never cross a sample line and cut no area from
/// any pixel row, and including them would put a crossing at every vertex of a flat edge.
pub(crate) struct Edge {
    pub(crate) y_min: f32,
    pub(crate) y_max: f32,
    x_at_y_min: f32,
    /// Change in x per unit of y, so a crossing is one multiply-add.
    inv_slope: f32,
    /// +1 where the segment runs up the image, -1 where it runs down, so that a ring
    /// wound counter-clockwise — the winding a contour calls material — counts positive.
    pub(crate) direction: i32,
}

impl Edge {
    fn new(from: Vec2, to: Vec2) -> Option<Self> {
        let direction = if to.y > from.y { -1 } else { 1 };
        let (low, high) = if to.y > from.y {
            (from, to)
        } else {
            (to, from)
        };
        // A segment with no vertical extent cannot be crossed by a sample line.
        if low.y >= high.y {
            return None;
        }
        Some(Self {
            y_min: low.y,
            y_max: high.y,
            x_at_y_min: low.x,
            inv_slope: (high.x - low.x) / (high.y - low.y),
            direction,
        })
    }

    pub(crate) fn x_at(&self, y: f32) -> f32 {
        (y - self.y_min).mul_add(self.inv_slope, self.x_at_y_min)
    }
}

/// Builds the edge table of a ring of points, closing edge included, sorted by the top
/// of each edge so the sweep can take them in order.
pub(crate) fn edges_of(rings: impl Iterator<Item = Vec<Vec2>>) -> Vec<Edge> {
    let mut edges = Vec::new();
    for ring in rings {
        for (index, from) in ring.iter().enumerate() {
            let to = ring[(index + 1) % ring.len()];
            edges.extend(Edge::new(*from, to));
        }
    }
    edges.sort_unstable_by(|a, b| a.y_min.total_cmp(&b.y_min));
    edges
}

/// Walks the image downwards, keeping the edges that currently matter.
///
/// One `Sweep` serves one caller: rows and sample lines must both be asked for in
/// increasing order, because it is a single pass over the edge table either way.
pub(crate) struct Sweep<'a> {
    edges: &'a [Edge],
    next: usize,
    active: Vec<usize>,
    crossings: Vec<(f32, i32)>,
}

impl<'a> Sweep<'a> {
    pub(crate) fn new(edges: &'a [Edge]) -> Self {
        Self {
            edges,
            next: 0,
            active: Vec::new(),
            crossings: Vec::new(),
        }
    }

    /// Spans of the sample line at height `y` that the positive winding rule calls solid.
    pub(crate) fn spans_at(&mut self, y: f32) -> impl Iterator<Item = (f32, f32)> + '_ {
        while let Some(edge) = self.edges.get(self.next) {
            if edge.y_min > y {
                break;
            }
            self.active.push(self.next);
            self.next += 1;
        }
        // An edge covers [y_min, y_max), so a vertex shared by two edges is crossed once.
        let edges = self.edges;
        self.active.retain(|&index| edges[index].y_max > y);

        self.crossings.clear();
        self.crossings.extend(
            self.active
                .iter()
                .map(|&index| (edges[index].x_at(y), edges[index].direction)),
        );
        self.crossings.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));

        spans(&self.crossings)
    }

    /// The edges crossing the pixel row `row`, that is `[row, row + 1)`.
    pub(crate) fn active_in_row(&mut self, row: u32) -> impl Iterator<Item = &Edge> + '_ {
        let (top, bottom) = (row as f32, (row + 1) as f32);
        while let Some(edge) = self.edges.get(self.next) {
            if edge.y_min >= bottom {
                break;
            }
            self.active.push(self.next);
            self.next += 1;
        }
        let edges = self.edges;
        self.active.retain(|&index| edges[index].y_max > top);

        self.active.iter().map(move |&index| &edges[index])
    }
}

/// Pairs crossings into solid spans by the positive winding rule: material runs from
/// where the winding number turns positive to where it stops being positive.
///
/// Positive rather than non-zero so that a body wound inward takes material away wherever
/// it lands, including outside the model; see `docs/decisions/0071`.
fn spans(crossings: &[(f32, i32)]) -> impl Iterator<Item = (f32, f32)> + '_ {
    let mut winding = 0;
    let mut start = 0.0;
    crossings.iter().filter_map(move |&(x, direction)| {
        let solid = winding > 0;
        if !solid {
            start = x;
        }
        winding += direction;
        (solid && winding <= 0).then_some((start, x))
    })
}

/// Coverage differences of one pixel row: at pixel `index`, the winding-weighted coverage
/// changes by `delta` and holds that value until the next difference.
///
/// The row is described by the handful of pixels an edge or a span touches, never by the
/// pixels in between, so a solid interior of any width is free. See
/// `docs/design/rasterisation.md`.
pub(crate) type Deltas = Vec<(u32, f32)>;

/// Adds a span as whole pixels, lighting those whose centre the span contains.
pub(crate) fn add_span_binary(deltas: &mut Deltas, width_px: u32, span: (f32, f32)) {
    let (start, end) = span;
    let first = (start - 0.5).ceil().max(0.0) as u32;
    let last = ((end - 0.5).ceil().max(0.0) as u32).min(width_px);
    if last > first {
        deltas.push((first, 1.0));
        deltas.push((last, -1.0));
    }
}

/// Turns the differences of one row into runs of equal grey and appends them to `out`,
/// leaving `deltas` empty for the next row.
///
/// `row_start` is the index of the row's first pixel within the layer, which is how the
/// builder learns about the dark rows and columns nothing was written to.
pub(crate) fn emit_row(
    deltas: &mut Deltas,
    width_px: u32,
    row_start: u32,
    grey: Grey,
    out: &mut RunsBuilder,
) {
    deltas.sort_unstable_by_key(|&(index, _)| index);

    let mut winding = 0.0f32;
    let mut cursor = 0;
    while let Some(&(index, _)) = deltas.get(cursor) {
        if index >= width_px {
            break;
        }
        // Every difference at this pixel lands before the pixel's value is read.
        while let Some(&(at, delta)) = deltas.get(cursor) {
            if at != index {
                break;
            }
            winding += delta;
            cursor += 1;
        }

        let next = deltas
            .get(cursor)
            .map_or(width_px, |&(at, _)| at.min(width_px));
        out.pad_to(row_start + index);
        out.push(next - index, shade(winding, grey));
    }
    deltas.clear();
}

/// Coverage this close to empty or to full is given the panel's own black and white.
///
/// A pixel the edge clips by half a percent is not a grey the resin can tell from white,
/// but as a value of its own it breaks the run it sits in into three chunks. Two
/// hundredths of a pixel is 0.4 micrometres on a 0.018 mm panel, and worth megabytes on a
/// real file; see `docs/decisions/0039`.
const SNAP: f32 = 0.02;

/// Coverage as the 8-bit grey the panel is given.
///
/// A winding of zero or less is air: what a body wound inward covers is what it takes
/// away, not what it exposes.
pub(crate) fn shade(winding: f32, grey: Grey) -> u8 {
    let coverage = winding.clamp(0.0, 1.0);
    if coverage <= SNAP {
        return 0x00;
    }
    if coverage >= 1.0 - SNAP {
        return 0xFF;
    }
    grey.shade(coverage)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runs::{LayerRuns, Run};

    fn ring(points: &[(f32, f32)]) -> Vec<Vec2> {
        points.iter().map(|&(x, y)| Vec2::new(x, y)).collect()
    }

    fn spans_at(edges: &[Edge], y: f32) -> Vec<(f32, f32)> {
        Sweep::new(edges).spans_at(y).collect()
    }

    fn square() -> Vec<Edge> {
        edges_of(std::iter::once(ring(&[
            (1.0, 1.0),
            (4.0, 1.0),
            (4.0, 5.0),
            (1.0, 5.0),
        ])))
    }

    #[test]
    fn a_square_crosses_a_sample_line_twice() {
        assert_eq!(spans_at(&square(), 3.0), vec![(1.0, 4.0)]);
    }

    #[test]
    fn a_sample_line_above_the_shape_has_no_spans() {
        assert!(spans_at(&square(), 0.5).is_empty());
    }

    #[test]
    fn a_hole_splits_the_line_into_two_spans() {
        let outer = ring(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]);
        // Reversed, so its winding cancels the outer ring's.
        let inner = ring(&[(4.0, 4.0), (4.0, 6.0), (6.0, 6.0), (6.0, 4.0)]);
        let edges = edges_of(vec![outer, inner].into_iter());

        assert_eq!(spans_at(&edges, 5.0), vec![(0.0, 4.0), (6.0, 10.0)]);
    }

    #[test]
    fn a_ring_wound_inward_on_its_own_encloses_nothing() {
        let inward = ring(&[(0.0, 0.0), (0.0, 10.0), (10.0, 10.0), (10.0, 0.0)]);
        let edges = edges_of(std::iter::once(inward));

        assert!(
            spans_at(&edges, 5.0).is_empty(),
            "a hole with no material around it is air"
        );
    }

    #[test]
    fn a_ring_wound_inward_cuts_past_the_shape_it_overlaps() {
        let outer = ring(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]);
        // Reaches out past the right-hand edge, the way a drain hole reaches out of a wall.
        let inward = ring(&[(6.0, 4.0), (6.0, 6.0), (14.0, 6.0), (14.0, 4.0)]);
        let edges = edges_of(vec![outer, inward].into_iter());

        assert_eq!(spans_at(&edges, 5.0), vec![(0.0, 6.0)]);
    }

    #[test]
    fn two_overlapping_shapes_make_one_span() {
        let left = ring(&[(0.0, 0.0), (6.0, 0.0), (6.0, 10.0), (0.0, 10.0)]);
        let right = ring(&[(4.0, 0.0), (10.0, 0.0), (10.0, 10.0), (4.0, 10.0)]);
        let edges = edges_of(vec![left, right].into_iter());

        assert_eq!(spans_at(&edges, 5.0), vec![(0.0, 10.0)]);
    }

    #[test]
    fn a_row_holds_the_edges_that_cross_it_and_no_others() {
        let edges = square();
        let mut sweep = Sweep::new(&edges);
        assert_eq!(
            sweep.active_in_row(0).count(),
            0,
            "the shape starts at y = 1"
        );
        assert_eq!(sweep.active_in_row(3).count(), 2, "one edge on each side");
        assert_eq!(sweep.active_in_row(6).count(), 0, "it ends at y = 5");
    }

    #[test]
    fn a_binary_span_lights_the_pixels_whose_centre_it_covers() {
        let mut deltas = Deltas::new();
        // Covers the centres of pixels 1, 2 and 3 but not 0 or 4.
        add_span_binary(&mut deltas, 6, (0.6, 3.9));
        let mut out = LayerRuns::builder(6, 1);
        emit_row(&mut deltas, 6, 0, Grey::default(), &mut out);

        assert_eq!(out.finish().to_mask().pixels(), &[0, 255, 255, 255, 0, 0]);
    }

    #[test]
    fn a_solid_interior_of_any_width_costs_one_run() {
        let mut deltas = Deltas::new();
        add_span_binary(&mut deltas, 1000, (0.0, 1000.0));
        let mut out = LayerRuns::builder(1000, 1);
        emit_row(&mut deltas, 1000, 0, Grey::default(), &mut out);

        assert_eq!(
            out.finish().runs(),
            &[Run {
                length: 1000,
                value: 255
            }]
        );
    }

    #[test]
    fn a_row_is_placed_where_its_start_says() {
        let mut deltas = Deltas::new();
        add_span_binary(&mut deltas, 4, (0.0, 2.0));
        let mut out = LayerRuns::builder(4, 3);
        emit_row(&mut deltas, 4, 4, Grey::default(), &mut out);

        assert_eq!(
            out.finish().to_mask().pixels(),
            &[0, 0, 0, 0, 255, 255, 0, 0, 0, 0, 0, 0]
        );
    }

    #[test]
    fn a_row_is_emptied_by_the_row_it_produced() {
        let mut deltas = Deltas::new();
        add_span_binary(&mut deltas, 4, (0.0, 2.0));
        let mut out = LayerRuns::builder(4, 1);
        emit_row(&mut deltas, 4, 0, Grey::default(), &mut out);
        assert!(deltas.is_empty());
    }
}
