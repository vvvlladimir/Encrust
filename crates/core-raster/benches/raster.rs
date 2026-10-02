#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use core_raster::{Grey, PixelPitch, RasterSettings, Rasterizer, ScanlineRasterizer, Shading};
use core_slicer::{Contour, Layer, Winding};
use criterion::{Criterion, criterion_group, criterion_main};
use glam::Vec2;

/// The panel of an Elegoo Mars 4 Ultra: 8520 x 4320 at a 0.018 mm pitch.
fn panel(shading: Shading) -> RasterSettings {
    RasterSettings {
        width_px: 8520,
        height_px: 4320,
        pitch: PixelPitch {
            x: 153.36 / 8520.0,
            y: 77.76 / 4320.0,
        },
        mirror_x: true,
        mirror_y: false,
        shading,
        grey: Grey::default(),
        blur_px: 0,
    }
}

/// A 30 mm disc in the middle of the plate, at the point count slicing a detailed mesh
/// produces.
fn layer() -> Layer {
    let centre = Vec2::new(76.68, 38.88);
    let points = 2000;
    let step = std::f32::consts::TAU / points as f32;
    let ring = (0..points)
        .map(|i| {
            let angle = step * i as f32;
            centre + Vec2::new(30.0 * angle.cos(), 30.0 * angle.sin())
        })
        .collect();

    Layer {
        z: 1.0,
        contours: vec![Contour::new(ring, Winding::Outer)],
        extra: Vec::new(),
    }
}

fn rasterising(c: &mut Criterion) {
    let layer = layer();
    let mut group = c.benchmark_group("raster");
    group.sample_size(10);

    for (name, shading) in [("coverage", Shading::Coverage), ("binary", Shading::Binary)] {
        let settings = panel(shading);
        group.bench_function(name, |b| {
            b.iter(|| {
                ScanlineRasterizer
                    .rasterize(&layer, &settings)
                    .expect("the panel request is sound")
            });
        });
    }
    for radius in [1, 2] {
        let settings = RasterSettings {
            blur_px: radius,
            ..panel(Shading::Coverage)
        };
        group.bench_function(format!("coverage_blur_{radius}"), |b| {
            b.iter(|| {
                ScanlineRasterizer
                    .rasterize(&layer, &settings)
                    .expect("the panel request is sound")
            });
        });
    }
    group.finish();
}

criterion_group!(benches, rasterising);
criterion_main!(benches);
