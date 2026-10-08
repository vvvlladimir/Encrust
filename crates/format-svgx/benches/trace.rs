use core_format::LayerSink;
use core_raster::{LayerMask, LayerRuns};
use criterion::{Criterion, criterion_group, criterion_main};
use format_svgx::SvgxSink;

/// The panel of an Elegoo Mars 4 Ultra: 8520 x 4320 pixels.
const WIDTH_PX: u32 = 8520;
const HEIGHT_PX: u32 = 4320;

/// A mask with `count` lit discs of `radius_px` in a row across the panel, which is what a
/// plate of round parts leaves: the outline the tracer follows grows with the count while
/// the dark surround stays the same.
fn mask(radius_px: f32, count: u32) -> LayerMask {
    let mut mask = LayerMask::new(WIDTH_PX, HEIGHT_PX);
    let width = WIDTH_PX as usize;
    let step = WIDTH_PX as f32 / (count + 1) as f32;
    let centres: Vec<(f32, f32)> = (0..count)
        .map(|index| ((index + 1) as f32 * step, HEIGHT_PX as f32 / 2.0))
        .collect();

    for (index, pixel) in mask.pixels_mut().iter_mut().enumerate() {
        let x = (index % width) as f32;
        let y = (index / width) as f32;
        if centres
            .iter()
            .any(|&(cx, cy)| (x - cx).hypot(y - cy) <= radius_px)
        {
            *pixel = 255;
        }
    }
    mask
}

fn tracing(c: &mut Criterion) {
    let mut group = c.benchmark_group("svgx_trace");
    group.sample_size(10);

    for (name, radius_px, count) in [
        ("blank", 0.0, 0),
        ("one_part", 555.0, 1),
        ("full_plate", 400.0, 8),
    ] {
        let layer = LayerRuns::from_mask(&mask(radius_px, count));
        group.bench_function(name, |b| {
            b.iter(|| <SvgxSink<'_> as LayerSink>::encode(&layer));
        });
    }
    group.finish();
}

criterion_group!(benches, tracing);
criterion_main!(benches);
