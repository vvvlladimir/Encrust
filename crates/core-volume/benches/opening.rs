#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use core_geometry::{Bvh, Vec3, ball};
use core_volume::{Cancel, HollowSettings, hollow};
use criterion::{Criterion, criterion_group, criterion_main};

/// What keeping the thin parts solid adds to a hollow, on the ball `field` hollows.
fn benchmarks(c: &mut Criterion) {
    let mesh = ball(Vec3::ZERO, 20.0, 320, 320);
    let bvh = Bvh::build(&mesh);
    let plain = HollowSettings {
        thickness_mm: 2.0,
        precision: 0.5,
        ..HollowSettings::default()
    };
    let kept = HollowSettings {
        solid_under_mm: Some(6.0),
        ..plain.clone()
    };
    let mut group = c.benchmark_group("keep solid under");
    group.sample_size(10);
    group.bench_function("hollow a 40 mm ball with a 2 mm wall", |b| {
        b.iter(|| hollow(std::hint::black_box(&mesh), &bvh, &plain, Cancel::never()));
    });
    group.bench_function("hollow it keeping what is under 6 mm solid", |b| {
        b.iter(|| {
            hollow(std::hint::black_box(&mesh), &bvh, &kept, Cancel::never())
                .expect("the ball hollows")
        });
    });
    group.finish();
}

criterion_group!(benches, benchmarks);
criterion_main!(benches);
