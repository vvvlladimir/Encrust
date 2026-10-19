#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use core_geometry::{Mesh, Scalar, Vec3, cuboid};
use core_volume::{Boolean, combine_exact};
use criterion::{Criterion, criterion_group, criterion_main};

/// A ball of `radius` at `centre`, `rings` by `segments` faces, wound outward.
fn ball(centre: Vec3, radius: Scalar, rings: usize, segments: usize) -> Mesh {
    let mut vertices = vec![centre + Vec3::new(0.0, 0.0, radius)];
    for ring in 1..rings {
        let theta = std::f32::consts::PI * ring as Scalar / rings as Scalar;
        for segment in 0..segments {
            let phi = std::f32::consts::TAU * segment as Scalar / segments as Scalar;
            let direction = Vec3::new(
                theta.sin() * phi.cos(),
                theta.sin() * phi.sin(),
                theta.cos(),
            );
            vertices.push(centre + direction * radius);
        }
    }
    let south = vertices.len() as u32;
    vertices.push(centre - Vec3::new(0.0, 0.0, radius));
    let at = |ring: usize, segment: usize| (1 + (ring - 1) * segments + segment % segments) as u32;
    let mut faces = Vec::new();
    for segment in 0..segments {
        faces.push([0, at(1, segment), at(1, segment + 1)]);
        faces.push([at(rings - 1, segment), south, at(rings - 1, segment + 1)]);
    }
    for ring in 1..rings - 1 {
        for segment in 0..segments {
            faces.push([
                at(ring, segment),
                at(ring + 1, segment),
                at(ring + 1, segment + 1),
            ]);
            faces.push([
                at(ring, segment),
                at(ring + 1, segment + 1),
                at(ring, segment + 1),
            ]);
        }
    }
    Mesh::new(vertices, faces)
}

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
