use core_geometry::{Mesh, Prominence, Vec3, ball};
use criterion::{Criterion, criterion_group, criterion_main};

/// About a million triangles: a 30 mm ball the size of a bust, with a hundred bumps of
/// detail over its top half.
fn bust() -> Mesh {
    let mut mesh = ball(Vec3::ZERO, 15.0, 500, 1000);
    for at in 0..100 {
        let along = at as f32 * 2.4;
        let up = 0.3 + 0.6 * (at as f32 / 100.0);
        let ring = (1.0 - up * up).sqrt();
        let on = Vec3::new(ring * along.cos(), ring * along.sin(), up) * 15.0;
        mesh.append(&ball(on, 0.6, 12, 24));
    }
    mesh
}

fn benchmarks(c: &mut Criterion) {
    let mesh = bust();
    assert!(
        mesh.faces.len() > 1_000_000,
        "the bust is a million triangles"
    );
    c.bench_function("prominence of a million-triangle bust", |b| {
        b.iter(|| Prominence::of(std::hint::black_box(&mesh)));
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = benchmarks
}
criterion_main!(benches);
