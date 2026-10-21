#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use core_geometry::{Bvh, Mat3, Mesh, Scalar, Vec3, ball, cuboid};
use core_volume::{
    Boolean, Cancel, HoleSize, HollowMode, HollowSettings, carve, cavity_field, combine_exact,
    drill, drilled, drilled_pieces, hole_at, plugs,
};
use criterion::{Criterion, criterion_group, criterion_main};

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

    // The cavity's field is built once: what is timed is the floor cut out of the shell.
    let dense = ball(Vec3::ZERO, 20.0, 512, 512);
    let settings = HollowSettings {
        thickness_mm: 2.0,
        mode: HollowMode::Internal,
        open_toward: Some(Vec3::NEG_Z),
        ..HollowSettings::default()
    };
    let field = cavity_field(
        &dense,
        &Bvh::build(&dense),
        &settings,
        Mat3::IDENTITY,
        Cancel::never(),
    )
    .expect("the ball has a cavity");
    let mut group = c.benchmark_group("open_floor");
    group.sample_size(10);
    group.bench_function("a dense ball opened through its floor", |b| {
        b.iter(|| carve(&dense, &field, &settings, Cancel::never()).expect("the ball opens"));
    });
    group.finish();
}

/// Three plugs on a half-million-face ball, what a change of the plug gap asks for.
fn plug_benchmarks(c: &mut Criterion) {
    let dense = ball(Vec3::ZERO, 20.0, 512, 512);
    let bvh = Bvh::build(&dense);
    let holes: Vec<_> = [Vec3::Z, Vec3::X, Vec3::NEG_Y]
        .into_iter()
        .map(|toward| {
            hole_at(&dense, &bvh, toward * 25.0, HoleSize::round(3.0, 3.0, 1.0))
                .expect("the ball has a surface")
        })
        .collect();
    let mut group = c.benchmark_group("plugs");
    group.sample_size(10);
    group.bench_function("three plugs on a dense ball", |b| {
        b.iter(|| plugs(&mut None, &dense, &bvh, &holes, None, 0.3).expect("the holes are sound"));
    });
    // What a second gap costs once the ball has been read round its holes for the first.
    let mut source = None;
    plugs(&mut source, &dense, &bvh, &holes, None, 0.3).expect("the holes are sound");
    group.bench_function("three plugs from a dense ball already read", |b| {
        b.iter(|| {
            plugs(&mut source, &dense, &bvh, &holes, None, 0.3).expect("the holes are sound")
        });
    });
    group.finish();
}

/// Three holes taken out of a half-million-face ball exactly, what each edit of its holes
/// asks the window for.
fn drill_benchmarks(c: &mut Criterion) {
    let dense = ball(Vec3::ZERO, 20.0, 512, 512);
    let bvh = Bvh::build(&dense);
    let holes: Vec<_> = [Vec3::Z, Vec3::X, Vec3::NEG_Y]
        .into_iter()
        .map(|toward| {
            hole_at(&dense, &bvh, toward * 25.0, HoleSize::round(3.0, 3.0, 1.0))
                .expect("the ball has a surface")
        })
        .collect();
    let cuts = drill(&holes, &[]).expect("the holes drill");
    let mut group = c.benchmark_group("drilled");
    group.sample_size(10);
    group.bench_function("three holes out of a dense ball", |b| {
        b.iter(|| drilled(&dense, &cuts).expect("a ball is a solid"));
    });
    // Twenty by twenty columns under a plate, a hole at two opposite corners: two reached.
    let mut forest = Mesh::default();
    for (row, column) in (0..20).flat_map(|row| (0..20).map(move |column| (row, column))) {
        let at = Vec3::new(5.0 * column as Scalar, 5.0 * row as Scalar, 0.0);
        forest.append(&ball(at, 1.0, 32, 32));
    }
    let corners = [Vec3::new(0.0, 0.0, 1.0), Vec3::new(95.0, 95.0, 1.0)];
    let bvh = Bvh::build(&forest);
    let holes: Vec<_> = corners
        .into_iter()
        .map(|at| {
            hole_at(&forest, &bvh, at + Vec3::Z, HoleSize::round(1.0, 1.0, 1.0))
                .expect("a column has a surface")
        })
        .collect();
    let cuts = drill(&holes, &[]).expect("the holes drill");
    group.bench_function("two holes through four hundred columns", |b| {
        b.iter(|| drilled_pieces(&forest, &cuts));
    });
    group.finish();
}

criterion_group!(boolean, benchmarks, plug_benchmarks, drill_benchmarks);
criterion_main!(boolean);
