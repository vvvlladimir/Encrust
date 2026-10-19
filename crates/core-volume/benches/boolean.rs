#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use core_geometry::{Mesh, Scalar, Vec3, ball, cuboid};
use core_volume::{Boolean, combine_exact};
use criterion::{Criterion, criterion_group, criterion_main};

/// A half-million-face ball with seven small blocks standing out of its top, the way a
/// line of letters stands on a dense model.
fn lettered() -> Vec<Mesh> {
    let mut models = vec![ball(Vec3::ZERO, 20.0, 512, 512)];
    models.extend((0..7).map(|letter| {
        let x = -10.5 + 3.0 * letter as Scalar;
        cuboid(Vec3::new(x, -3.0, 19.0), Vec3::new(x + 2.0, 3.0, 21.0))
    }));
    models
}

fn benchmarks(c: &mut Criterion) {
    let models = lettered();
    let mut group = c.benchmark_group("combine_exact");
    group.sample_size(10);
    group.bench_function("seven letters raised on a dense ball", |b| {
        b.iter(|| combine_exact(&models, Boolean::Union).expect("the letters join the ball"));
    });
    group.bench_function("seven letters sunk into a dense ball", |b| {
        b.iter(|| combine_exact(&models, Boolean::Difference).expect("the letters cut the ball"));
    });
    group.finish();
}

criterion_group!(boolean, benchmarks);
criterion_main!(boolean);
