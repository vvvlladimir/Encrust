use core_geometry::{Bvh, Mesh, Transform, Vec3, ball};
use core_plate::{Body, BodyKind, crossings};
use criterion::{Criterion, criterion_group, criterion_main};

fn model<'a>(owner: usize, mesh: &'a Mesh, bvh: &'a Bvh, at_mm: Vec3) -> Body<'a> {
    Body {
        owner,
        kind: BodyKind::Model,
        mesh,
        bvh: Some(bvh),
        transform: Transform {
            translation: at_mm,
            ..Transform::default()
        },
    }
}

/// Two balls of a quarter of a million faces each, the second moved `apart_mm` along X:
/// in place it is a duplicate whose every face crosses its twin, a seam all over.
fn two_balls(c: &mut Criterion) {
    let mesh = ball(Vec3::ZERO, 30.0, 250, 500);
    let bvh = Bvh::build(&mesh);
    let mut group = c.benchmark_group("crossings");
    group.sample_size(10);
    for (name, apart_mm) in [
        ("duplicate-in-place", 0.0),
        ("duplicate-nudged-0.05mm", 0.05),
        ("half-overlapping", 30.0),
    ] {
        let bodies = [
            model(0, &mesh, &bvh, Vec3::ZERO),
            model(1, &mesh, &bvh, Vec3::X * apart_mm),
        ];
        group.bench_function(name, |b| {
            b.iter(|| {
                let met = crossings(&bodies);
                assert_eq!(met.len(), 1, "the two balls meet");
                met
            });
        });
    }
    group.finish();
}

criterion_group!(benches, two_balls);
criterion_main!(benches);
