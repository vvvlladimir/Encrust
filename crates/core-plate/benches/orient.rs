#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use core_geometry::{Mesh, Scalar, Vec3};
use core_plate::{OrientSettings, orient};
use criterion::{Criterion, criterion_group, criterion_main};

/// A closed sphere of `rings` by `segments` quads, which is the shape orientation is
/// worst at: no flat face to rest on, so every candidate of the sphere is measured.
fn ball(radius_mm: Scalar, rings: usize, segments: usize) -> Mesh {
    let mut vertices = Vec::with_capacity(rings * segments + 2);
    vertices.push(Vec3::new(0.0, 0.0, radius_mm));
    for ring in 1..rings {
        let polar = std::f32::consts::PI * ring as Scalar / rings as Scalar;
        for segment in 0..segments {
            let azimuth = std::f32::consts::TAU * segment as Scalar / segments as Scalar;
            vertices.push(Vec3::new(
                radius_mm * polar.sin() * azimuth.cos(),
                radius_mm * polar.sin() * azimuth.sin(),
                radius_mm * polar.cos(),
            ));
        }
    }
    vertices.push(Vec3::new(0.0, 0.0, -radius_mm));

    let bottom = (vertices.len() - 1) as u32;
    let at = |ring: usize, segment: usize| -> u32 { 1 + ((ring - 1) * segments + segment) as u32 };
    let mut faces = Vec::new();
    for segment in 0..segments {
        let next = (segment + 1) % segments;
        faces.push([0, at(1, segment), at(1, next)]);
        faces.push([bottom, at(rings - 1, next), at(rings - 1, segment)]);
    }
    for ring in 1..rings - 1 {
        for segment in 0..segments {
            let next = (segment + 1) % segments;
            faces.push([at(ring, segment), at(ring + 1, segment), at(ring, next)]);
            faces.push([at(ring, next), at(ring + 1, segment), at(ring + 1, next)]);
        }
    }
    Mesh::new(vertices, faces)
}

fn orienting(c: &mut Criterion) {
    let mesh = ball(30.0, 160, 320);
    let settings = OrientSettings::default();

    let mut group = c.benchmark_group("orient");
    group.sample_size(10);
    group.bench_function("ball-100k-faces", |b| {
        b.iter(|| orient(&mesh, &settings).expect("a ball can be oriented"));
    });
    group.finish();
}

criterion_group!(benches, orienting);
criterion_main!(benches);
