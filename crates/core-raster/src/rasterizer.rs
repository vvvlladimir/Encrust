use std::ops::Range;

use core_slicer::{Contour, Layer};
use glam::Vec2;

use crate::area::add_edge_row;
use crate::pixels::PixelSpace;
use crate::runs::{LayerRuns, RunsBuilder};
use crate::scanline::{Deltas, Edge, Sweep, add_span_binary, edges_of, emit_row, shade};
use crate::{Grey, RasterError, RasterSettings, Rastered, Shading};

/// Converts the contours of one layer into an exposure mask.
pub trait Rasterizer {
    fn rasterize(&self, layer: &Layer, settings: &RasterSettings) -> Result<Rastered, RasterError>;
}

/// Fills contours by sweeping sample lines down the panel. See
/// `docs/design/rasterisation.md`.
#[derive(Debug, Default, Clone, Copy)]
pub struct ScanlineRasterizer;

impl Rasterizer for ScanlineRasterizer {
    fn rasterize(&self, layer: &Layer, settings: &RasterSettings) -> Result<Rastered, RasterError> {
        settings.validate()?;

        // A blur fades the exact coverage, so the range and the ladder wait until after it;
        // see `docs/decisions/0117`.
        let blur_px = match settings.shading {
            Shading::Coverage => settings.blur_px,
            Shading::Binary => 0,
        };
        let planes = RasterSettings {
            grey: if blur_px == 0 {
                settings.grey
            } else {
                Grey::default()
            },
            ..*settings
        };

        let mut rastered = one_plane(&layer.contours, &planes);
        // Each extra plane is filled on its own and the brighter pixel kept, because
        // adding their windings would call a wall covered twice fully exposed; see
        // `docs/decisions/0114`.
        for contours in &layer.extra {
            let plane = one_plane(contours, &planes);
            rastered.runs = rastered.runs.brightest_of(&plane.runs);
            rastered.overflow_px = rastered.overflow_px.max(plane.overflow_px);
        }
        if blur_px > 0 {
            rastered.runs = graded(&rastered.runs.blurred(blur_px), settings.grey);
        }
        Ok(rastered)
    }
}

/// Runs of linear coverage written as the grey the panel is given.
fn graded(runs: &LayerRuns, grey: Grey) -> LayerRuns {
    let mut out = LayerRuns::builder(runs.width(), runs.height());
    for run in runs.runs() {
        out.push(run.length, shade(f32::from(run.value) / 255.0, grey));
    }
    out.finish()
}

/// One sampled plane's contours as a mask of its own.
fn one_plane(contours: &[Contour], settings: &RasterSettings) -> Rastered {
    let space = PixelSpace::new(settings);
    let reversed = space.reverses_winding();
    let rings: Vec<Vec<Vec2>> = contours
        .iter()
        .map(|contour| {
            let mut ring: Vec<Vec2> = contour.points.iter().map(|&p| space.map(p)).collect();
            if reversed {
                ring.reverse();
            }
            ring
        })
        .collect();
    let overflow_px = overflow_of(&rings, settings);
    let mut runs = LayerRuns::builder(settings.width_px, settings.height_px);

    // A layer starts dark, so one that touches no pixel is already finished: its
    // builder closes as a single dark run over the whole panel.
    if let Some(rows) = touched_rows(&rings, settings) {
        let edges = edges_of(rings.into_iter());
        fill(&mut runs, &edges, rows, settings);
    }

    Rastered {
        runs: runs.finish(),
        overflow_px,
    }
}

/// The rows the layer can reach, clamped to the panel, or `None` when it reaches none. Other rows
/// stay dark and are padded in one run: cost follows the part, not the panel.
fn touched_rows(rings: &[Vec<Vec2>], settings: &RasterSettings) -> Option<Range<u32>> {
    let (min, max) = rings
        .iter()
        .flatten()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), point| {
            (min.min(point.y), max.max(point.y))
        });
    if min > max {
        return None;
    }

    let extent = settings.height_px as f32;
    let start = min.floor().clamp(0.0, extent) as u32;
    let end = max.ceil().clamp(0.0, extent) as u32;
    (start < end).then_some(start..end)
}

fn fill(out: &mut RunsBuilder, edges: &[Edge], rows: Range<u32>, settings: &RasterSettings) {
    match settings.shading {
        Shading::Coverage => fill_by_area(out, edges, rows, settings),
        Shading::Binary => fill_by_centre(out, edges, rows, settings),
    }
}

/// Grey in proportion to the exact area of each pixel the layer covers.
fn fill_by_area(
    out: &mut RunsBuilder,
    edges: &[Edge],
    rows: Range<u32>,
    settings: &RasterSettings,
) {
    let width_px = settings.width_px;
    let mut sweep = Sweep::new(edges);
    let mut deltas = Deltas::new();

    for row in rows {
        for edge in sweep.active_in_row(row) {
            add_edge_row(&mut deltas, width_px, row, edge);
        }
        emit_row(&mut deltas, width_px, row * width_px, settings.grey, out);
    }
}

/// Whole pixels, lit where the layer covers the centre of the row.
fn fill_by_centre(
    out: &mut RunsBuilder,
    edges: &[Edge],
    rows: Range<u32>,
    settings: &RasterSettings,
) {
    let width_px = settings.width_px;
    let mut sweep = Sweep::new(edges);
    let mut deltas = Deltas::new();

    for row in rows {
        for span in sweep.spans_at(row as f32 + 0.5) {
            add_span_binary(&mut deltas, width_px, span);
        }
        emit_row(&mut deltas, width_px, row * width_px, settings.grey, out);
    }
}

/// How far past the panel the layer reaches, in pixels, over all four sides.
fn overflow_of(rings: &[Vec<Vec2>], settings: &RasterSettings) -> f32 {
    let panel = Vec2::new(settings.width_px as f32, settings.height_px as f32);
    rings
        .iter()
        .flatten()
        .map(|point| {
            let past_low = -point.min(Vec2::ZERO);
            let past_high = (*point - panel).max(Vec2::ZERO);
            past_low.max(past_high).max_element()
        })
        .fold(0.0, f32::max)
}

#[cfg(test)]
mod tests;
