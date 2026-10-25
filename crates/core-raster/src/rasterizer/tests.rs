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
