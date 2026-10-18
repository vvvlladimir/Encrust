#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use core_csg::Solid;
use core_geometry::{Mesh, Scalar, Vec3, cylinder};
use core_volume::{Cancel, part_off};
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

fn solid(mesh: &Mesh) -> Solid {
    Solid::new(mesh).expect("a ball or a bar is a solid")
}

fn benchmarks(c: &mut Criterion) {
    // Two 20 mm balls, about a quarter of a million faces, joined by a 4 mm bar.
    let left = ball(Vec3::ZERO, 20.0, 256, 256);
    let right = ball(Vec3::new(60.0, 0.0, 0.0), 20.0, 256, 256);
    let bar = cylinder(
        Vec3::new(15.0, 0.0, 0.0),
        Vec3::new(45.0, 0.0, 0.0),
        2.0,
        64,
    );
    let dumbbell = solid(&left)
        .union(&solid(&bar))
        .and_then(|joined| joined.union(&solid(&right)))
        .expect("the balls and the bar join")
        .mesh();
    // A 6 mm ball half a millimetre off a 20 mm one: no wave leaves it.
    let mut apart = ball(Vec3::ZERO, 20.0, 256, 256);
    apart.append(&ball(Vec3::new(26.5, 0.0, 0.0), 6.0, 64, 64));

    let mut group = c.benchmark_group("part_off");
    group.sample_size(10);
    group.bench_function("a ball off a dumbbell at its bar", |b| {
        b.iter(|| {
            part_off(&dumbbell, Vec3::new(-20.0, 0.0, 0.0), Cancel::never())
                .expect("the bar is a neck")
        });
    });
    group.bench_function("a ball no material joins, cut out", |b| {
        b.iter(|| {
            part_off(&apart, Vec3::new(32.5, 0.0, 0.0), Cancel::never())
                .expect("the small ball is apart")
        });
    });
    group.finish();
}

criterion_group!(parts, benchmarks);
criterion_main!(parts);
