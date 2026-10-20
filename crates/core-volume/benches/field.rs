#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use core_geometry::{Bvh, Heightmap, Mesh, Region, Scalar, Transform, UvMap, Vec2, Vec3, ball};
use core_volume::{
    Cancel, FieldSettings, HollowMode, HollowSettings, InfillPattern, InfillSettings, Panel,
    ReliefSettings, SignMode, StackField, build, extract, hollow, offset, press, press_painted,
    shell,
};
use criterion::{Criterion, criterion_group, criterion_main};

fn benchmarks(c: &mut Criterion) {
    let mesh = ball(Vec3::ZERO, 20.0, 320, 320);
    let bvh = Bvh::build(&mesh);
    let coarse = FieldSettings {
        voxel_mm: 0.4,
        ..FieldSettings::default()
    };
    let fine = FieldSettings::default();
    let by_winding = FieldSettings {
        sign: SignMode::Winding,
        ..coarse
    };

    let field = build(&mesh, &bvh, &fine, Cancel::never()).expect("the sphere has faces");

    c.bench_function("build a 0.4 mm field of a 40 mm ball", |b| {
        b.iter(|| build(std::hint::black_box(&mesh), &bvh, &coarse, Cancel::never()));
    });

    c.bench_function("build a 0.1 mm field of a 40 mm ball", |b| {
        b.iter(|| build(std::hint::black_box(&mesh), &bvh, &fine, Cancel::never()));
    });

    c.bench_function("build a 0.4 mm field, signed by winding number", |b| {
        b.iter(|| {
            build(
                std::hint::black_box(&mesh),
                &bvh,
                &by_winding,
                Cancel::never(),
            )
        });
    });

    // A wall is the case hollowing actually builds, and the one that costs: the band sits
    // a whole thickness off the mesh, where a gather ball is the wall wide.
    let wall = FieldSettings {
        voxel_mm: 0.1,
        iso_mm: -2.0,
        ..FieldSettings::default()
    };
    c.bench_function("build a 0.1 mm field 2 mm inside a 40 mm ball", |b| {
        b.iter(|| build(std::hint::black_box(&mesh), &bvh, &wall, Cancel::never()));
    });

    c.bench_function("build that wall signed by winding number", |b| {
        b.iter(|| {
            build(
                std::hint::black_box(&mesh),
                &bvh,
                &FieldSettings {
                    sign: SignMode::Winding,
                    ..wall
                },
                Cancel::never(),
            )
        });
    });

    c.bench_function("offset a 0.1 mm field inward", |b| {
        b.iter(|| offset(std::hint::black_box(&field), -0.1));
    });

    c.bench_function("shell a 0.1 mm field", |b| {
        b.iter(|| shell(std::hint::black_box(&field), 0.15));
    });

    c.bench_function("extract a 0.1 mm field", |b| {
        b.iter(|| extract(std::hint::black_box(&field), Cancel::never()));
    });

    hollowing(c, &mesh, &bvh);
    stacking(c);
}

/// A sliced file read back: a 40 mm disc 20 mm tall on a 16K panel, 400 layers.
fn stacking(c: &mut Criterion) {
    let panel = Panel {
        width_px: 11_520,
        height_px: 5_120,
        pitch_mm: (0.019, 0.019),
    };
    let disc = disc_runs(panel, 20.0);
    let mut group = c.benchmark_group("stack");
    group.sample_size(10);
    group.bench_function("field_of_a_disc_400_layers_0.05mm", |b| {
        b.iter(|| {
            let mut field = StackField::new(panel, 0.05);
            for layer in 0..400 {
                let bottom = layer as Scalar * 0.05;
                field.push(bottom, bottom + 0.05, std::hint::black_box(&disc));
            }
            field.finish()
        });
    });
    group.finish();
}

/// Runs lighting a disc of `radius_mm` in the middle of `panel`.
fn disc_runs(panel: Panel, radius_mm: Scalar) -> Vec<(u32, u8)> {
    let (width, height) = (panel.width_px, panel.height_px);
    let centre = Vec2::new(width as Scalar, height as Scalar) * 0.5;
    let radius_px = radius_mm / panel.pitch_mm.0;
    let mut runs = Vec::new();
    let mut dark = 0;
    for y in 0..height {
        let dy = y as Scalar + 0.5 - centre.y;
        let half = (radius_px * radius_px - dy * dy).max(0.0).sqrt() as u32;
        if half == 0 {
            dark += width;
            continue;
        }
        let first = centre.x as u32 - half;
        runs.push((dark + first, 0));
        runs.push((2 * half, 255));
        dark = width - first - 2 * half;
    }
    runs.push((dark, 0));
    runs
}

/// What a cavity, its infill and a relief cost on the same ball.
fn hollowing(c: &mut Criterion, mesh: &Mesh, bvh: &Bvh) {
    let empty = HollowSettings {
        thickness_mm: 2.0,
        mode: HollowMode::Internal,
        precision: 0.5,
        ..HollowSettings::default()
    };
    let filled = HollowSettings {
        infill: Some(InfillSettings::default()),
        ..empty.clone()
    };

    c.bench_function("hollow a 40 mm ball with a 2 mm wall", |b| {
        b.iter(|| hollow(std::hint::black_box(mesh), bvh, &empty, Cancel::never()));
    });

    c.bench_function("hollow a 40 mm ball and fill it with a hive", |b| {
        b.iter(|| hollow(std::hint::black_box(mesh), bvh, &filled, Cancel::never()));
    });

    // A wall a few voxels thick on a mesh of faces far larger than a voxel, as a coarse
    // figurine is: every voxel of the band is too near the mesh to trust what was carried.
    let coarse = ball(Vec3::ZERO, 20.0, 64, 64);
    let coarse_bvh = Bvh::build(&coarse);
    let thin = HollowSettings {
        thickness_mm: 0.15,
        ..empty.clone()
    };
    let mut group = c.benchmark_group("thin wall");
    group.sample_size(10);
    group.bench_function(
        "hollow an 8k-triangle 40 mm ball with a 0.15 mm wall",
        |b| {
            b.iter(|| {
                hollow(
                    std::hint::black_box(&coarse),
                    &coarse_bvh,
                    &thin,
                    Cancel::never(),
                )
            });
        },
    );
    group.finish();

    infill(c, mesh, bvh, &empty);

    // A relief is a field of its own plus a nearest-point query per lattice point, so it
    // is priced against the build it is built on.
    let spherical = spherical_uvs(mesh);
    let checks = Heightmap::new(64, 64, checkerboard(64)).expect("one sample per pixel");
    let relief = ReliefSettings {
        amplitude_mm: 0.4,
        precision: 0.5,
        ..ReliefSettings::default()
    };
    c.bench_function("press a 0.4 mm relief into a 40 mm ball", |b| {
        b.iter(|| {
            press(
                std::hint::black_box(mesh),
                bvh,
                &spherical,
                std::slice::from_ref(&checks),
                &relief,
            )
        });
    });

    // Painted, only the patch is split and moved: priced against the field press above.
    let mut patch = Region::default();
    patch.brush(
        (mesh, bvh, Transform::default()),
        Vec3::new(0.0, 0.0, 20.0),
        8.0,
        true,
    );
    c.bench_function(
        "press a 0.4 mm relief into an 8 mm patch of a 40 mm ball",
        |b| {
            b.iter(|| {
                press_painted(
                    std::hint::black_box(mesh),
                    &spherical,
                    std::slice::from_ref(&checks),
                    &relief,
                    &patch,
                    Cancel::never(),
                )
            });
        },
    );

    // Half a million triangles is the size of a scanned bust, and what the step aimed at.
    let dense = ball(Vec3::ZERO, 20.0, 500, 500);
    let dense_bvh = Bvh::build(&dense);
    c.bench_function("hollow a 500k-triangle ball with a 2 mm wall", |b| {
        b.iter(|| {
            hollow(
                std::hint::black_box(&dense),
                &dense_bvh,
                &empty,
                Cancel::never(),
            )
        });
    });
}

/// Each pattern at 6 mm cells and 45%, about 2 mm walls, on the ball and on one big enough
/// that the lattice rather than the shell is what costs.
fn infill(c: &mut Criterion, mesh: &Mesh, bvh: &Bvh, empty: &HollowSettings) {
    let filled = |pattern| HollowSettings {
        infill: Some(InfillSettings {
            pattern,
            cell_mm: 6.0,
            density: 0.45,
        }),
        ..empty.clone()
    };
    let grid = filled(InfillPattern::Grid);
    let big = ball(Vec3::ZERO, 50.0, 320, 320);
    let big_bvh = Bvh::build(&big);

    let mut group = c.benchmark_group("infill");
    group.sample_size(10);
    group.bench_function("grid_in_a_40mm_ball", |b| {
        b.iter(|| hollow(std::hint::black_box(mesh), bvh, &grid, Cancel::never()));
    });
    group.bench_function("empty_100mm_ball", |b| {
        b.iter(|| hollow(std::hint::black_box(&big), &big_bvh, empty, Cancel::never()));
    });
    group.bench_function("grid_in_a_100mm_ball", |b| {
        b.iter(|| hollow(std::hint::black_box(&big), &big_bvh, &grid, Cancel::never()));
    });
    for (name, pattern) in [
        ("hive_in_a_100mm_ball", InfillPattern::Hive),
        ("scaffold_in_a_100mm_ball", InfillPattern::Scaffold),
    ] {
        let settings = filled(pattern);
        group.bench_function(name, |b| {
            b.iter(|| {
                hollow(
                    std::hint::black_box(&big),
                    &big_bvh,
                    &settings,
                    Cancel::never(),
                )
            });
        });
    }
    group.finish();
}

/// A latitude and longitude UV for every corner of every face.
fn spherical_uvs(mesh: &Mesh) -> UvMap {
    let at = |vertex: Vec3| {
        let direction = vertex.normalize_or_zero();
        Vec2::new(
            direction.y.atan2(direction.x) / std::f32::consts::TAU + 0.5,
            direction.z * 0.5 + 0.5,
        )
    };
    UvMap::whole(
        mesh.faces
            .iter()
            .map(|face| face.map(|index| at(mesh.vertices[index as usize])))
            .collect(),
    )
}

/// Alternating black and white pixels, the worst a relief can be asked for.
fn checkerboard(side: usize) -> Vec<Scalar> {
    (0..side * side)
        .map(|index| ((index / side + index % side) % 2) as Scalar)
        .collect()
}

criterion_group!(benches, benchmarks);
criterion_main!(benches);
