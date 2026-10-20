#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use core_csg::Solid;
use core_geometry::{Mesh, Vec3, ball, cuboid};
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::time::Duration;

/// A ball of radius 20 mm with about `faces` faces.
fn dense_ball(faces: usize) -> Mesh {
    let rings = ((faces as f64 / 2.0).sqrt()).round() as usize;
    ball(Vec3::ZERO, 20.0, rings, rings)
}

fn joined(parts: &[Mesh]) -> Mesh {
    let mut out = Mesh::default();
    for part in parts {
        let base = out.vertices.len() as u32;
        out.vertices.extend_from_slice(&part.vertices);
        out.faces
            .extend(part.faces.iter().map(|face| face.map(|at| at + base)));
    }
    out
}

/// A 10 x 10 grid of 1 mm square bars through the ball.
fn prisms() -> Mesh {
    let bars: Vec<Mesh> = (0..100)
        .map(|at| {
            let (x, y) = (
                -15.0 + 3.0 * (at % 10) as f32,
                -15.0 + 3.0 * (at / 10) as f32,
            );
            cuboid(Vec3::new(x, y, -30.0), Vec3::new(x + 1.0, y + 1.0, 30.0))
        })
        .collect();
    joined(&bars)
}

fn read(c: &mut Criterion) {
    let mut group = c.benchmark_group("read");
    group
        .sample_size(10)
        .measurement_time(Duration::from_secs(10));
    for faces in [200_000, 1_000_000] {
        let mesh = dense_ball(faces);
        group.bench_with_input(BenchmarkId::from_parameter(faces), &mesh, |b, mesh| {
            b.iter(|| Solid::new(mesh).expect("a ball is a solid"));
        });
    }
    group.finish();
}

fn boolean(c: &mut Criterion) {
    let mut group = c.benchmark_group("boolean");
    group
        .sample_size(10)
        .measurement_time(Duration::from_secs(10));
    let ball = Solid::new(&dense_ball(1_000_000)).expect("a ball is a solid");
    let bars = Solid::new(&prisms()).expect("bars are solids");
    group.bench_function("ball-minus-prisms-1m", |b| {
        b.iter(|| {
            ball.difference(&bars)
                .expect("the bars cross the ball")
                .mesh()
        });
    });
    group.finish();
}

/// `solids` unioned two by two down a tree, as callers did before `union_all`.
fn folded(solids: &[Solid]) -> Solid {
    match solids {
        [] => Solid::empty(),
        [only] => only.clone(),
        _ => {
            let (left, right) = solids.split_at(solids.len() / 2);
            let (left, right) = rayon::join(|| folded(left), || folded(right));
            left.union(&right).expect("the parts are solids")
        }
    }
}

fn union_many(c: &mut Criterion) {
    let mut group = c.benchmark_group("union-many");
    group
        .sample_size(10)
        .measurement_time(Duration::from_secs(10));
    // Letters and parts apart from each other: 200 balls of 2 k faces in a 20 x 10 grid.
    let apart: Vec<Solid> = (0..200)
        .map(|at| {
            let centre = Vec3::new(5.0 * (at % 20) as f32, 5.0 * (at / 20) as f32, 0.0);
            Solid::new(&ball(centre, 2.0, 32, 32)).expect("a ball is a solid")
        })
        .collect();
    // Struts crossing each other: 20 bars along x through 20 along y.
    let crossing: Vec<Solid> = (0..40)
        .map(|at| {
            let off = -19.0 + 2.0 * (at % 20) as f32;
            let (min, max) = if at < 20 {
                (Vec3::new(-20.0, off, 0.0), Vec3::new(20.0, off + 0.5, 1.0))
            } else {
                (Vec3::new(off, -20.0, 0.0), Vec3::new(off + 0.5, 20.0, 1.0))
            };
            Solid::new(&cuboid(min, max)).expect("a bar is a solid")
        })
        .collect();
    for (name, solids) in [("apart-200", &apart), ("crossing-40", &crossing)] {
        group.bench_function(BenchmarkId::new("tree", name), |b| {
            b.iter(|| folded(solids));
        });
        group.bench_function(BenchmarkId::new("union-all", name), |b| {
            b.iter(|| Solid::union_all(solids).expect("the parts are solids"));
        });
    }
    group.finish();
}

criterion_group!(benches, read, boolean, union_many);
criterion_main!(benches);
