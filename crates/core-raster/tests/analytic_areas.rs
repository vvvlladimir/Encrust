#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use std::num::NonZeroU8;

use core_raster::{
    Grey, LayerMask, PixelPitch, RasterSettings, Rasterizer, ScanlineRasterizer, Shading,
};
use core_slicer::{Contour, Layer, Winding};
use glam::Vec2;

const PITCH: f32 = 0.1;

fn settings(shading: Shading) -> RasterSettings {
    RasterSettings {
        width_px: 200,
        height_px: 200,
        pitch: PixelPitch { x: PITCH, y: PITCH },
        mirror_x: false,
        mirror_y: false,
        shading,
        grey: Grey::default(),
        blur_px: 0,
    }
}

/// A regular `points`-gon of radius `radius` millimetres, wound counter-clockwise.
/// Its area is exactly `points / 2 * radius^2 * sin(2 pi / points)`.
fn polygon(centre: Vec2, radius: f32, points: usize) -> (Layer, f32) {
    let step = std::f32::consts::TAU / points as f32;
    let ring: Vec<Vec2> = (0..points)
        .map(|i| {
            let angle = step * i as f32;
            centre + Vec2::new(radius * angle.cos(), radius * angle.sin())
        })
        .collect();
    let area = points as f32 / 2.0 * radius * radius * step.sin();

    (
        Layer {
            z: 1.0,
            contours: vec![Contour::new(ring, Winding::Outer)],
            extra: Vec::new(),
        },
        area,
    )
}

fn mask(layer: &Layer, shading: Shading) -> LayerMask {
    ScanlineRasterizer
        .rasterize(layer, &settings(shading))
        .expect("the request is sound")
        .runs
        .to_mask()
}

fn mask_with(layer: &Layer, grey: Grey) -> LayerMask {
    let settings = RasterSettings {
        grey,
        ..settings(Shading::Coverage)
    };
    ScanlineRasterizer
        .rasterize(layer, &settings)
        .expect("the request is sound")
        .runs
        .to_mask()
}

/// Area the mask exposes, in square millimetres.
fn exposed_mm2(mask: &LayerMask) -> f32 {
    mask.coverage() * PITCH * PITCH
}

#[test]
fn a_disc_exposes_the_area_its_polygon_encloses() {
    let (layer, expected) = polygon(Vec2::new(10.03, 9.97), 5.03, 720);
    let found = exposed_mm2(&mask(&layer, Shading::default()));

    let error = (found - expected).abs() / expected;
    assert!(
        error < 1e-4,
        "a 720-gon of radius 5.03 mm encloses {expected:.4} mm^2, got {found:.4} mm^2"
    );
}

#[test]
fn anti_aliasing_is_an_order_of_magnitude_more_accurate_than_binary() {
    let (layer, expected) = polygon(Vec2::new(10.03, 9.97), 5.03, 720);

    let error = |shading| (exposed_mm2(&mask(&layer, shading)) - expected).abs() / expected;
    let smooth = error(Shading::default());
    let binary = error(Shading::Binary);

    assert!(
        smooth * 10.0 < binary,
        "coverage shading was off by {smooth:.5}, binary by {binary:.5}"
    );
}

#[test]
fn exact_area_coverage_is_accurate_in_both_directions() {
    // A near-horizontal edge is what sub-scanline sampling quantised worst. A thin
    // wedge is almost all near-horizontal edge, so its area is the strongest statement
    // about the vertical direction the rasteriser can make.
    let wedge = Layer {
        z: 1.0,
        contours: vec![Contour::new(
            vec![
                Vec2::new(2.0, 2.0),
                Vec2::new(14.0, 2.0),
                Vec2::new(14.0, 2.37),
            ],
            Winding::Outer,
        )],
        extra: Vec::new(),
    };
    // Half of a 12 mm by 0.37 mm rectangle.
    let expected = 0.5 * 12.0 * 0.37;
    let found = exposed_mm2(&mask(&wedge, Shading::default()));

    assert!(
        (found - expected).abs() / expected < 1e-3,
        "a wedge of {expected:.4} mm^2, got {found:.4} mm^2"
    );
}

#[test]
fn a_hole_stays_dark_and_costs_its_own_area() {
    let (mut layer, outer) = polygon(Vec2::new(10.0, 10.0), 6.0, 360);
    let (hole, hole_area) = polygon(Vec2::new(10.0, 10.0), 2.0, 360);

    // Reversed, the way core-slicer winds a hole.
    let mut points = hole.contours[0].points.clone();
    points.reverse();
    layer.contours.push(Contour::new(points, Winding::Inner));

    let found = exposed_mm2(&mask(&layer, Shading::default()));
    let expected = outer - hole_area;
    assert!(
        (found - expected).abs() / expected < 1e-3,
        "ring of {expected:.4} mm^2, got {found:.4} mm^2"
    );

    // The centre of the hole must be fully dark.
    let centre = (10.0 / PITCH) as usize;
    assert_eq!(
        mask(&layer, Shading::default()).pixels()[centre * 200 + centre],
        0
    );
}

#[test]
fn two_overlapping_bodies_expose_their_union_not_their_difference() {
    let (left, area) = polygon(Vec2::new(8.0, 10.0), 4.0, 360);
    let (right, _) = polygon(Vec2::new(11.0, 10.0), 4.0, 360);
    let both = Layer {
        z: 1.0,
        contours: vec![left.contours[0].clone(), right.contours[0].clone()],
        extra: Vec::new(),
    };

    let found = exposed_mm2(&mask(&both, Shading::default()));
    assert!(
        found > area && found < area * 2.0,
        "two overlapping discs of {area:.3} mm^2 must expose their union, got {found:.3}"
    );
    // Even-odd would darken the lens where they overlap; non-zero keeps it lit.
    let centre_row = (10.0 / PITCH) as usize * 200;
    assert_eq!(
        mask(&both, Shading::default()).pixels()[centre_row + (95)],
        255
    );
}

#[test]
fn the_mask_starts_at_the_near_corner_of_the_plate() {
    // A square at the plate origin must land in the first row a file carries.
    let layer = Layer {
        z: 1.0,
        contours: vec![Contour::new(
            vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(2.0, 0.0),
                Vec2::new(2.0, 2.0),
                Vec2::new(0.0, 2.0),
            ],
            Winding::Outer,
        )],
        extra: Vec::new(),
    };
    let mask = mask(&layer, Shading::default());

    assert_eq!(
        mask.pixels()[0],
        255,
        "the first pixel of the first row is lit"
    );
    assert_eq!(mask.pixels()[199 * 200], 0, "the last row is not");
}

#[test]
fn no_pixel_is_written_a_grey_the_panel_would_not_cure() {
    // A circle off the pixel grid: its edge covers pixels by every fraction there is.
    let (layer, _) = polygon(Vec2::new(10.03, 10.07), 5.0, 256);
    let floored = mask_with(
        &layer,
        Grey {
            floor: 128,
            levels: None,
        },
    );

    assert!(
        floored.pixels().iter().all(|&p| p == 0 || p >= 128),
        "an edge pixel is either dark or bright enough to hold"
    );
    assert!(
        mask(&layer, Shading::Coverage)
            .pixels()
            .iter()
            .any(|&p| (1..128).contains(&p)),
        "without a floor the same circle does write greys that would not cure"
    );
}

#[test]
fn rounding_to_eight_greys_leaves_only_the_rungs() {
    let (layer, _) = polygon(Vec2::new(10.03, 10.07), 5.0, 256);
    let rounded = mask_with(
        &layer,
        Grey {
            floor: 0,
            levels: NonZeroU8::new(8),
        },
    );

    let rungs = [0, 31, 63, 95, 127, 159, 191, 223, 255];
    assert!(
        rounded.pixels().iter().all(|p| rungs.contains(p)),
        "every pixel sits on a rung of the eight-level ladder"
    );
}

#[test]
fn an_extra_plane_lights_what_the_layer_own_plane_missed() {
    let (mut layer, disc_mm2) = polygon(Vec2::new(10.0, 10.0), 5.0, 256);
    let (fin, fin_mm2) = polygon(Vec2::new(17.0, 10.0), 1.0, 64);
    layer.extra.push(fin.contours);

    let both = mask(&layer, Shading::Coverage);
    let expected = disc_mm2 + fin_mm2;
    assert!(
        (exposed_mm2(&both) - expected).abs() / expected < 0.01,
        "the union of the two planes is {} mm^2, not {expected}",
        exposed_mm2(&both)
    );
}

#[test]
fn two_planes_over_the_same_wall_do_not_brighten_its_edge() {
    let (mut once, area) = polygon(Vec2::new(10.03, 10.07), 5.0, 256);
    let alone = mask(&once, Shading::Coverage);
    once.extra.push(once.contours.clone());
    let twice = mask(&once, Shading::Coverage);

    assert_eq!(
        twice.pixels(),
        alone.pixels(),
        "a wall sampled twice exposes the same {area} mm^2, not a fatter edge"
    );
}
