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

        // A blur fades the exact coverage, so the floor and the ladder wait until after it;
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

/// The rows the layer can reach, clamped to the panel, or `None` when it reaches none.
///
/// Rows outside stay dark, so the sweep never visits them and the builder pads over them
/// in one run: the cost follows the part, not the panel.
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
mod tests {
    use core_slicer::{Contour, Winding};

    use super::*;
    use crate::{LayerMask, PixelPitch};

    fn fully_lit(mask: &LayerMask) -> usize {
        mask.pixels()
            .iter()
            .fold(0, |count, &pixel| count + usize::from(pixel == 255))
    }

    fn settings() -> RasterSettings {
        RasterSettings {
            width_px: 10,
            height_px: 10,
            pitch: PixelPitch { x: 1.0, y: 1.0 },
            mirror_x: false,
            mirror_y: false,
            shading: Shading::default(),
            grey: Grey::default(),
            blur_px: 0,
        }
    }

    fn layer_of(points: Vec<Vec2>) -> Layer {
        Layer {
            z: 0.5,
            contours: vec![Contour::new(points, Winding::Outer)],
            extra: Vec::new(),
        }
    }

    /// A square from (2, 2) to (6, 6) millimetres, which is a whole number of pixels.
    fn square_layer() -> Layer {
        layer_of(vec![
            Vec2::new(2.0, 2.0),
            Vec2::new(6.0, 2.0),
            Vec2::new(6.0, 6.0),
            Vec2::new(2.0, 6.0),
        ])
    }

    fn mask_of(layer: &Layer, settings: &RasterSettings) -> LayerMask {
        ScanlineRasterizer
            .rasterize(layer, settings)
            .expect("valid request")
            .runs
            .to_mask()
    }

    #[test]
    fn a_square_on_the_pixel_grid_has_no_grey_edge() {
        let rastered = ScanlineRasterizer
            .rasterize(&square_layer(), &settings())
            .expect("valid request");
        let mask = rastered.runs.to_mask();

        assert!(
            mask.pixels().iter().all(|&p| p == 0 || p == 255),
            "a square aligned to the grid covers whole pixels only"
        );
        assert_eq!(
            fully_lit(&mask),
            16,
            "a 4 x 4 mm square at a 1 mm pitch lights 16 pixels"
        );
        assert!(rastered.fits());
    }

    #[test]
    fn an_empty_layer_is_one_dark_run_over_the_whole_panel() {
        let rastered = ScanlineRasterizer
            .rasterize(&Layer::empty(0.5), &settings())
            .expect("valid request");

        assert!(rastered.runs.is_blank());
        assert_eq!(rastered.runs.runs().len(), 1);
    }

    #[test]
    fn a_half_pixel_edge_comes_out_half_lit() {
        let layer = layer_of(vec![
            Vec2::new(2.0, 2.0),
            Vec2::new(5.5, 2.0),
            Vec2::new(5.5, 6.0),
            Vec2::new(2.0, 6.0),
        ]);
        let mask = mask_of(&layer, &settings());

        // Column 5 is covered from 5.0 to 5.5, so it exposes at half.
        let row = 5 * 10;
        assert_eq!(mask.pixels()[row + 4], 255);
        assert_eq!(mask.pixels()[row + 5], 128);
    }

    #[test]
    fn binary_shading_leaves_no_grey_at_all() {
        let layer = layer_of(vec![
            Vec2::new(2.0, 2.0),
            Vec2::new(5.5, 2.0),
            Vec2::new(5.5, 6.0),
            Vec2::new(2.0, 6.0),
        ]);
        let rastered = ScanlineRasterizer
            .rasterize(
                &layer,
                &RasterSettings {
                    shading: Shading::Binary,
                    grey: Grey::default(),
                    ..settings()
                },
            )
            .expect("valid request");

        assert!(
            rastered
                .runs
                .runs()
                .iter()
                .all(|run| run.value == 0 || run.value == 255)
        );
    }

    #[test]
    fn mirroring_reflects_the_mask() {
        let plain = mask_of(&square_layer(), &settings());
        let mirrored = mask_of(
            &square_layer(),
            &RasterSettings {
                mirror_x: true,
                ..settings()
            },
        );

        let flipped: Vec<u8> = plain
            .pixels()
            .as_chunks::<10>()
            .0
            .iter()
            .flat_map(|row| row.iter().rev().copied())
            .collect();
        assert_eq!(mirrored.pixels(), flipped.as_slice());
    }

    #[test]
    fn a_layer_reaching_past_the_panel_is_clipped_and_reported() {
        let layer = layer_of(vec![
            Vec2::new(-3.0, 2.0),
            Vec2::new(6.0, 2.0),
            Vec2::new(6.0, 6.0),
            Vec2::new(-3.0, 6.0),
        ]);
        let rastered = ScanlineRasterizer
            .rasterize(&layer, &settings())
            .expect("valid request");

        assert!(!rastered.fits());
        assert!((rastered.overflow_px - 3.0).abs() < 1e-6);
        assert_eq!(
            fully_lit(&rastered.runs.to_mask()),
            24,
            "the 6 mm that landed on the panel, 4 rows tall"
        );
    }

    #[test]
    fn the_runs_of_a_layer_add_up_to_the_panel() {
        let rastered = ScanlineRasterizer
            .rasterize(&square_layer(), &settings())
            .expect("valid request");

        let total: u32 = rastered.runs.runs().iter().map(|run| run.length).sum();
        assert_eq!(total, rastered.runs.pixel_count());
    }

    fn blurred(grey: Grey, shading: Shading) -> LayerMask {
        mask_of(
            &square_layer(),
            &RasterSettings {
                grey,
                shading,
                blur_px: 1,
                ..settings()
            },
        )
    }

    #[test]
    fn a_blurred_edge_fades_either_side_of_where_it_was() {
        let mask = blurred(Grey::default(), Shading::Coverage);

        // Row 3 is inside the square top to bottom, which spans columns 2 to 5.
        let row = &mask.pixels()[3 * 10..4 * 10];
        assert_eq!(row, &[0, 85, 170, 255, 255, 170, 85, 0, 0, 0]);
        let area = LayerRuns::from_mask(&mask).coverage();
        assert!(
            (area - 16.0).abs() < 0.1,
            "a box filter moves exposure, it does not add any: {area}"
        );
    }

    #[test]
    fn the_floor_cuts_a_blurred_edge_after_the_fade() {
        let grey = Grey {
            floor: 128,
            levels: None,
        };
        let row = blurred(grey, Shading::Coverage).pixels()[3 * 10..4 * 10].to_vec();
        assert_eq!(
            row,
            vec![0, 0, 170, 255, 255, 170, 0, 0, 0, 0],
            "the outer third would not cure; the inner two thirds would"
        );
    }

    #[test]
    fn binary_shading_ignores_a_blur() {
        let mask = blurred(Grey::default(), Shading::Binary);
        assert!(mask.pixels().iter().all(|&p| p == 0 || p == 255));
    }

    #[test]
    fn a_broken_request_is_rejected_before_any_work() {
        let broken = RasterSettings {
            width_px: 0,
            ..settings()
        };
        assert!(matches!(
            ScanlineRasterizer.rasterize(&square_layer(), &broken),
            Err(RasterError::ZeroResolution { .. })
        ));
    }

    #[test]
    fn a_panel_with_more_pixels_than_an_index_can_hold_is_rejected() {
        let huge = RasterSettings {
            width_px: 100_000,
            height_px: 100_000,
            ..settings()
        };
        assert!(matches!(
            ScanlineRasterizer.rasterize(&square_layer(), &huge),
            Err(RasterError::PanelTooLarge { .. })
        ));
    }
}
