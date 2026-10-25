use core_geometry::{Mesh, Vec3, ball, cylinder, smooth_normals};
use criterion::{Criterion, criterion_group, criterion_main};

/// Roughly a million triangles of twelve-sided struts with round joints, the size of a
/// figurine's supports.
fn struts() -> Mesh {
    let mut mesh = Mesh::default();
    for strut in 0..6000 {
        let foot = Vec3::new((strut % 80) as f32 * 3.0, (strut / 80) as f32 * 3.0, 0.0);
        let head = foot + Vec3::new(0.5, 0.3, 8.0);
        mesh.append(&cylinder(foot, head, 0.4, 12));
        mesh.append(&ball(head, 0.4, 6, 12));
    }
    mesh
}

fn benchmarks(c: &mut Criterion) {
    let mesh = struts();
    c.bench_function("smooth the normals of a million triangles of struts", |b| {
        b.iter(|| smooth_normals(std::hint::black_box(&mesh), 50.0));
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = benchmarks
}
criterion_main!(benches);
