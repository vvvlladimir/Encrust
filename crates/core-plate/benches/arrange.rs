#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use core_geometry::{Mesh, Scalar, Transform, Vec2, Vec3};
use core_plate::{ArrangeSettings, Footprint, arrange};
use criterion::{Criterion, criterion_group, criterion_main};

/// A star-shaped footprint: concave, so packing cannot fall back on bounding boxes.
fn star(radius_mm: Scalar, points: usize) -> Mesh {
    let mut vertices = vec![Vec3::ZERO, Vec3::new(0.0, 0.0, 10.0)];
    for index in 0..points * 2 {
        let angle = std::f32::consts::TAU * index as Scalar / (points * 2) as Scalar;
        let reach = if index % 2 == 0 {
            radius_mm
        } else {
            radius_mm / 2.5
        };
        vertices.push(Vec3::new(reach * angle.cos(), reach * angle.sin(), 0.0));
        vertices.push(Vec3::new(reach * angle.cos(), reach * angle.sin(), 10.0));
    }

    let mut faces = Vec::new();
    let corners = points * 2;
    for index in 0..corners {
        let low = 2 + 2 * index as u32;
        let next_low = 2 + 2 * ((index + 1) % corners) as u32;
        faces.push([0, next_low, low]);
        faces.push([1, low + 1, next_low + 1]);
        faces.push([low, next_low, low + 1]);
        faces.push([next_low, next_low + 1, low + 1]);
    }
    Mesh::new(vertices, faces)
}

fn packing(c: &mut Criterion) {
    let mesh = star(18.0, 5);
    let settings = ArrangeSettings::default();
    let footprints: Vec<Footprint> = (0..8)
        .map(|_| {
            Footprint::of(&mesh, Transform::default(), settings.cell_mm)
                .expect("a star has a footprint")
        })
        .collect();

    let mut group = c.benchmark_group("arrange");
    group.bench_function("eight-stars-on-a-mars-4-plate", |b| {
        b.iter(|| arrange(&footprints, Vec2::new(153.4, 77.0), &settings));
    });
    group.finish();
}

criterion_group!(benches, packing);
criterion_main!(benches);
