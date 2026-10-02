#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use core_analysis::{Measured, cure, measure};
use core_raster::{Grey, PixelPitch, RasterSettings, Rasterizer, ScanlineRasterizer, Shading};
use core_slicer::{Contour, Layer, Winding};
use criterion::{Criterion, criterion_group, criterion_main};
use glam::Vec2;

/// The panel of an Elegoo Mars 4 Ultra: 8520 x 4320 at a 0.018 mm pitch.
fn panel() -> RasterSettings {
    RasterSettings {
        width_px: 8520,
        height_px: 4320,
        pitch: PixelPitch {
            x: 153.36 / 8520.0,
            y: 77.76 / 4320.0,
        },
        mirror_x: true,
        mirror_y: false,
        shading: Shading::Coverage,
        grey: Grey::default(),
        blur_px: 0,
    }
}

/// A grid of 3 mm rings across the plate: many pieces and many runs a row, the costly
/// case for joining spans.
fn layer() -> Layer {
    let ring = |centre: Vec2, radius: f32, winding: Winding| {
        let points = 200;
        let step = std::f32::consts::TAU / points as f32;
        let mut ring: Vec<Vec2> = (0..points)
            .map(|i| centre + Vec2::from_angle(step * i as f32) * radius)
            .collect();
        if winding == Winding::Inner {
            ring.reverse();
        }
        Contour::new(ring, winding)
    };
    let mut contours = Vec::new();
    for column in 0..30 {
        for row in 0..15 {
            let centre = Vec2::new(4.0 + column as f32 * 5.0, 4.0 + row as f32 * 5.0);
            contours.push(ring(centre, 2.0, Winding::Outer));
            contours.push(ring(centre, 1.2, Winding::Inner));
        }
    }
    Layer::new(1.0, contours)
}

fn measuring(c: &mut Criterion) {
    let settings = panel();
    let layer = layer();
    let runs = ScanlineRasterizer
        .rasterize(&layer, &settings)
        .expect("the rings fit on the panel")
        .runs;
    let mut group = c.benchmark_group("analysis");
    group.sample_size(20);
    // What the measure adds to writing a layer is judged against producing it.
    group.bench_function("rasterise", |b| {
        b.iter(|| ScanlineRasterizer.rasterize(&layer, &settings));
    });
    group.bench_function("measure", |b| b.iter(|| measure(&runs, settings.pitch)));
    // A layer folded over one like it: the contact walk, necks and levers.
    let cured = cure(&runs, settings.pitch);
    group.bench_function("fold", |b| {
        b.iter(|| {
            let mut measured = Measured::new(0);
            measured.push(cured.clone(), 0.05);
            measured.push(cured.clone(), 0.05);
            measured
        });
    });
    group.finish();
}

criterion_group!(benches, measuring);
criterion_main!(benches);
