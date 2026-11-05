#![expect(clippy::expect_used, reason = "a bench over bodies it builds itself")]

use std::ops::ControlFlow;

use core_geometry::{Adjacency, Bvh, Mat4, Mesh, SurfacePoint, Vec3, ball, cuboid, surface_path};
use criterion::{Criterion, criterion_group, criterion_main};

/// Roughly 200k triangles, the size of a detailed miniature.
fn sample() -> Mesh {
    ball(Vec3::ZERO, 20.0, 320, 320)
}

fn on(bvh: &Bvh, point: Vec3) -> SurfacePoint {
    let closest = bvh.closest(point).expect("the body has faces");
    SurfacePoint {
        face: closest.face,
        point: closest.point,
    }
}

fn benchmarks(c: &mut Criterion) {
    let mesh = sample();
    let adjacency = Adjacency::of(&mesh);
    let bvh = Bvh::build(&mesh);
    let walk = |from: Vec3, to: Vec3| {
        surface_path(
            &mesh,
            &adjacency,
            Mat4::IDENTITY,
            on(&bvh, from),
            on(&bvh, to),
            &mut |_| ControlFlow::Continue(()),
        )
    };

    c.bench_function("walk 200k triangles, a few millimetres", |b| {
        b.iter(|| walk(Vec3::new(20.0, 0.0, 0.0), Vec3::new(19.0, 6.0, 0.0)));
    });
    c.bench_function("walk 200k triangles, a quarter round", |b| {
        b.iter(|| walk(Vec3::new(20.0, 0.0, 0.0), Vec3::new(0.0, 20.0, 0.0)));
    });
    c.bench_function("walk 200k triangles, half round", |b| {
        b.iter(|| walk(Vec3::new(20.0, 0.0, 0.0), Vec3::new(-20.0, 0.0, 1.0)));
    });

    let mut apart = sample();
    apart.append(&cuboid(Vec3::splat(30.0), Vec3::splat(31.0)));
    let adjacency = Adjacency::of(&apart);
    let bvh = Bvh::build(&apart);
    let (from, to) = (on(&bvh, Vec3::X * 20.0), on(&bvh, Vec3::splat(31.0)));
    c.bench_function("walk 200k triangles to a shell apart", |b| {
        b.iter(|| {
            surface_path(&apart, &adjacency, Mat4::IDENTITY, from, to, &mut |_| {
                ControlFlow::Continue(())
            })
        });
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = benchmarks
}
criterion_main!(benches);
