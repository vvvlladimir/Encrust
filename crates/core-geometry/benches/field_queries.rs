#[path = "../tests/shared/bodies.rs"]
mod bodies;

use core_geometry::{Bvh, Mesh, Vec3, Winding, closest_point, winding_number};
use criterion::{Criterion, criterion_group, criterion_main};

/// Roughly 200k triangles, the size of a detailed miniature.
fn sample() -> Mesh {
    bodies::uv_sphere(20.0, 320, 320)
}

/// Points straddling the surface within a few tenths of a millimetre, which is where a
/// narrow-band field is evaluated and nowhere else.
fn band_probes(count: usize) -> Vec<Vec3> {
    (0..count)
        .map(|i| {
            let along = i as f32 / count as f32;
            let (sin_theta, cos_theta) = (along * std::f32::consts::PI).sin_cos();
            let (sin_phi, cos_phi) = (along * 37.0).sin_cos();
            let offset = (along * 53.0).sin() * 0.3;
            Vec3::new(sin_theta * cos_phi, sin_theta * sin_phi, cos_theta) * (20.0 + offset)
        })
        .collect()
}

fn benchmarks(c: &mut Criterion) {
    let mesh = sample();
    let bvh = Bvh::build(&mesh);
    let winding = Winding::build(&mesh);
    let points = band_probes(1024);

    c.bench_function("build a winding hierarchy over 200k triangles", |b| {
        b.iter(|| Winding::build(std::hint::black_box(&mesh)));
    });

    c.bench_function("nearest point on 200k triangles, every face", |b| {
        b.iter(|| closest_point(std::hint::black_box(&mesh), points[0]));
    });

    c.bench_function("nearest point on 200k triangles, hierarchy", |b| {
        b.iter(|| bvh.closest(std::hint::black_box(&mesh), points[0]));
    });

    c.bench_function("winding number of 200k triangles, every face", |b| {
        b.iter(|| winding_number(std::hint::black_box(&mesh), points[0]));
    });

    c.bench_function("winding number of 200k triangles, hierarchy", |b| {
        b.iter(|| winding.at(std::hint::black_box(&mesh), points[0]));
    });

    c.bench_function("1024 band voxels, distance and sign", |b| {
        b.iter(|| {
            points
                .iter()
                .map(|point| {
                    let distance = bvh
                        .closest(std::hint::black_box(&mesh), *point)
                        .map_or(0.0, |found| found.distance);
                    if winding.is_inside(&mesh, *point) {
                        -distance
                    } else {
                        distance
                    }
                })
                .sum::<f32>()
        });
    });
}

criterion_group!(benches, benchmarks);
criterion_main!(benches);
