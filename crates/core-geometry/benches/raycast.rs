#[path = "../tests/shared/bodies.rs"]
mod bodies;

use core_geometry::{Bvh, Mesh, Ray, Vec3, raycast};
use criterion::{Criterion, criterion_group, criterion_main};

/// Roughly 200k triangles, the size of a detailed miniature.
fn sample() -> Mesh {
    bodies::uv_sphere(20.0, 320, 320)
}

/// Rays straight down over the body, the way a support column looks for what to stand on.
fn drops(count: usize) -> Vec<Ray> {
    (0..count)
        .map(|i| {
            let along = i as f32 / count as f32;
            Ray::new(
                Vec3::new(along * 30.0 - 15.0, along * 14.0 - 7.0, 40.0),
                -Vec3::Z,
            )
        })
        .collect()
}

fn benchmarks(c: &mut Criterion) {
    let mesh = sample();
    let bvh = Bvh::build(&mesh);
    let rays = drops(64);

    c.bench_function("build a hierarchy over 200k triangles", |b| {
        b.iter(|| Bvh::build(std::hint::black_box(&mesh)));
    });

    c.bench_function("raycast 200k triangles, every face", |b| {
        b.iter(|| raycast(std::hint::black_box(&mesh), &rays[0]));
    });

    c.bench_function("raycast 200k triangles, hierarchy", |b| {
        b.iter(|| bvh.raycast(std::hint::black_box(&mesh), &rays[0]));
    });

    c.bench_function("64 support drops, every face", |b| {
        b.iter(|| {
            rays.iter()
                .filter_map(|ray| raycast(std::hint::black_box(&mesh), ray))
                .count()
        });
    });

    c.bench_function("64 support drops, hierarchy", |b| {
        b.iter(|| {
            rays.iter()
                .filter_map(|ray| bvh.raycast(std::hint::black_box(&mesh), ray))
                .count()
        });
    });
}

criterion_group!(benches, benchmarks);
criterion_main!(benches);
