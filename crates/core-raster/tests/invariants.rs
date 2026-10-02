#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use core_raster::{Grey, PixelPitch, RasterSettings, Rasterizer, ScanlineRasterizer, Shading};
use core_slicer::{Contour, Layer, Winding};
use glam::Vec2;
use proptest::prelude::*;

const PITCH: f32 = 0.1;
const SIDE_PX: u32 = 200;
/// Half the panel in millimetres; the panel is square, so rotating about it stays on it.
const CENTRE: f32 = SIDE_PX as f32 * PITCH / 2.0;

fn settings(shading: Shading) -> RasterSettings {
    RasterSettings {
        width_px: SIDE_PX,
        height_px: SIDE_PX,
        pitch: PixelPitch { x: PITCH, y: PITCH },
        mirror_x: false,
        mirror_y: false,
        shading,
        grey: Grey::default(),
        blur_px: 0,
    }
}

fn square(centre: Vec2, half: f32) -> Layer {
    Layer {
        z: 1.0,
        contours: vec![Contour::new(
            vec![
                centre + Vec2::new(-half, -half),
                centre + Vec2::new(half, -half),
                centre + Vec2::new(half, half),
                centre + Vec2::new(-half, half),
            ],
            Winding::Outer,
        )],
        extra: Vec::new(),
    }
}

fn coverage(layer: &Layer, shading: Shading) -> f32 {
    ScanlineRasterizer
        .rasterize(layer, &settings(shading))
        .expect("the request is sound")
        .runs
        .coverage()
}

/// Turns the layer a quarter turn about the middle of the panel.
fn quarter_turn(layer: &Layer) -> Layer {
    let middle = Vec2::splat(CENTRE);
    Layer {
        z: layer.z,
        contours: layer
            .contours
            .iter()
            .map(|contour| {
                let points = contour
                    .points
                    .iter()
                    .map(|point| {
                        let offset = *point - middle;
                        middle + Vec2::new(-offset.y, offset.x)
                    })
                    .collect();
                Contour::new(points, contour.winding)
            })
            .collect(),
        extra: Vec::new(),
    }
}

proptest! {
    /// A quarter turn on a square panel moves the same area to different pixels.
    #[test]
    fn a_quarter_turn_exposes_the_same_area(half in 0.5..8.0f32, offset in -4.0..4.0f32) {
        let layer = square(Vec2::new(CENTRE + offset, CENTRE - offset), half);
        let before = coverage(&layer, Shading::default());
        let after = coverage(&quarter_turn(&layer), Shading::default());

        prop_assert!((before - after).abs() < before * 1e-3 + 1.0);
    }

    /// Doubling a square's side quadruples what it exposes.
    #[test]
    fn area_grows_with_the_square_of_the_side(half in 0.5..4.0f32) {
        let small = coverage(&square(Vec2::splat(CENTRE), half), Shading::default());
        let large = coverage(&square(Vec2::splat(CENTRE), half * 2.0), Shading::default());

        // A sub-scanline is 1/16 of a pixel row, so a horizontal edge can be off by that
        // much on every column it spans. That error follows the perimeter, not the area,
        // and comparing a square against one twice its size cannot absorb it into a
        // relative bound.
        let side_px = half * 2.0 / PITCH;
        let edge_error = |side: f32| 2.0 * side / 16.0;
        let bound = large * 1e-3 + edge_error(side_px * 2.0) + 4.0 * edge_error(side_px);

        prop_assert!((large - small * 4.0).abs() < bound);
    }

    /// Whatever is drawn, the panel cannot expose more than all of itself.
    #[test]
    fn nothing_exposes_more_than_the_whole_panel(half in 0.5..100.0f32, offset in -50.0..50.0f32) {
        let layer = square(Vec2::splat(CENTRE + offset), half);
        let exposed = coverage(&layer, Shading::default());

        prop_assert!(exposed <= (SIDE_PX * SIDE_PX) as f32);
    }

    /// Binary shading is what a panel that ignores grey will see.
    #[test]
    fn binary_shading_writes_only_black_and_white(half in 0.5..8.0f32) {
        let layer = ScanlineRasterizer
            .rasterize(&square(Vec2::splat(CENTRE), half), &settings(Shading::Binary))
            .expect("the request is sound")
            .runs;

        prop_assert!(layer.runs().iter().all(|run| run.value == 0 || run.value == 255));
    }
}
