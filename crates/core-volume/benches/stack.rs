use core_volume::{Panel, StackField};
use criterion::{Criterion, criterion_group, criterion_main};

/// A 16K panel of 0.019 mm pixels, 218.9 x 97.3 mm.
const PANEL: Panel = Panel {
    width_px: 11_520,
    height_px: 5_120,
    pitch_mm: (0.019, 0.019),
};

/// Runs over `PANEL` lighting a square `side_px` wide at each of `corners`, row by row.
fn squares(corners: &[(u32, u32)], side_px: u32) -> Vec<(u32, u8)> {
    let mut runs = Vec::new();
    let mut dark = 0;
    for y in 0..PANEL.height_px {
        let mut x = 0;
        let mut lit: Vec<u32> = corners
            .iter()
            .filter(|(_, y0)| (*y0..*y0 + side_px).contains(&y))
            .map(|(x0, _)| *x0)
            .collect();
        lit.sort_unstable();
        for x0 in lit {
            runs.push((dark + x0 - x, 0));
            runs.push((side_px, 255));
            dark = 0;
            x = x0 + side_px;
        }
        dark += PANEL.width_px - x;
    }
    runs.push((dark, 0));
    runs
}

/// Six 10 mm parts spread over the whole plate, so their bounding box is the panel: what a
/// full plate of small models costs, 64 layers of 0.05 mm.
fn benchmarks(c: &mut Criterion) {
    let side_px = 526;
    let corners = [
        (100, 100),
        (5_500, 100),
        (10_800, 100),
        (100, 4_400),
        (5_500, 4_400),
        (10_800, 4_400),
    ];
    let runs = squares(&corners, side_px);
    let mut group = c.benchmark_group("stack field");
    group.sample_size(10);
    group.bench_function("64 layers of six parts spread over a 16K plate", |b| {
        b.iter(|| {
            let mut field = StackField::new(PANEL, 0.05);
            for layer in 0..64 {
                let bottom_mm = layer as f32 * 0.05;
                field.push(bottom_mm, bottom_mm + 0.05, std::hint::black_box(&runs));
            }
            field.finish()
        });
    });
    group.finish();
}

criterion_group!(benches, benchmarks);
criterion_main!(benches);
