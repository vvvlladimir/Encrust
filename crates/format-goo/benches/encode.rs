use core_raster::{LayerMask, LayerRuns};
use criterion::{Criterion, criterion_group, criterion_main};
use format_goo::EncodedLayer;

/// The panel of an Elegoo Mars 4 Ultra: 8520 x 4320 pixels.
const WIDTH_PX: u32 = 8520;
const HEIGHT_PX: u32 = 4320;

/// A mask with a lit disc of `radius_px` in the middle, the shape a layer of a round part
/// leaves. The dark surround is most of the panel, and is what the encoder spends its time
/// walking over.
fn mask(radius_px: f32) -> LayerMask {
    let mut mask = LayerMask::new(WIDTH_PX, HEIGHT_PX);
    let (centre_x, centre_y) = (WIDTH_PX as f32 / 2.0, HEIGHT_PX as f32 / 2.0);
    let width = WIDTH_PX as usize;

    for (index, pixel) in mask.pixels_mut().iter_mut().enumerate() {
        let x = (index % width) as f32 - centre_x;
        let y = (index / width) as f32 - centre_y;
        if x.hypot(y) <= radius_px {
            *pixel = 255;
        }
    }
    mask
}

fn encoding(c: &mut Criterion) {
    let mut group = c.benchmark_group("goo_encode");
    group.sample_size(20);

    for (name, radius_px) in [("blank", 0.0), ("small_part", 555.0), ("wide_part", 1666.0)] {
        let layer = LayerRuns::from_mask(&mask(radius_px));
        group.bench_function(name, |b| b.iter(|| EncodedLayer::encode(&layer)));
    }
    group.finish();
}

criterion_group!(benches, encoding);
criterion_main!(benches);
