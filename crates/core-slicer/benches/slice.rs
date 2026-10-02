#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

#[path = "../tests/shared/bodies.rs"]
mod shared;

use core_geometry::Mesh;
use core_slicer::{PlaneSliceEngine, SliceEngine, SliceSettings};
use criterion::{Criterion, criterion_group, criterion_main};
use shared::bodies;

/// Roughly 80k triangles over 40 mm of height: a detailed miniature at print resolution.
fn sample() -> Mesh {
    bodies::uv_sphere(20.0, 200, 200)
}

fn slicing(c: &mut Criterion) {
    let mesh = sample();
    let mut group = c.benchmark_group("slice");
    group.sample_size(10);

    for layer_height in [0.1, 0.05] {
        group.bench_function(format!("sphere_{layer_height}mm"), |b| {
            b.iter(|| {
                PlaneSliceEngine
                    .slice(
                        &mesh,
                        &SliceSettings {
                            layer_height,
                            ..SliceSettings::default()
                        },
                    )
                    .expect("the sphere is sliceable")
            });
        });
    }
    group.finish();
}

criterion_group!(benches, slicing);
criterion_main!(benches);
