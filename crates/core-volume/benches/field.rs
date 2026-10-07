#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use core_geometry::{Bvh, Heightmap, Mesh, Scalar, UvMap, Vec2, Vec3};
use core_volume::{
    Cancel, FieldSettings, HollowMode, HollowSettings, InfillPattern, InfillSettings,
    ReliefSettings, SignMode, build, extract, hollow, offset, press, shell,
};
use criterion::{Criterion, criterion_group, criterion_main};

/// Roughly 200k triangles, the size of a detailed miniature.
fn sphere(radius: Scalar, rings: usize, segments: usize) -> Mesh {
    let mut vertices = vec![Vec3::new(0.0, 0.0, radius)];
    for ring in 1..rings {
        let theta = std::f32::consts::PI * ring as Scalar / rings as Scalar;
        let (sin_theta, cos_theta) = theta.sin_cos();
        for segment in 0..segments {
            let phi = std::f32::consts::TAU * segment as Scalar / segments as Scalar;
            let (sin_phi, cos_phi) = phi.sin_cos();
            vertices.push(Vec3::new(
                radius * sin_theta * cos_phi,
                radius * sin_theta * sin_phi,
                radius * cos_theta,
            ));
        }
    }
    let south = vertices.len() as u32;
    vertices.push(Vec3::new(0.0, 0.0, -radius));

    let at = |ring: usize, segment: usize| (1 + (ring - 1) * segments + segment % segments) as u32;
    let mut faces = Vec::new();
    for segment in 0..segments {
        faces.push([0, at(1, segment), at(1, segment + 1)]);
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
    for segment in 0..segments {
        faces.push([at(rings - 1, segment), south, at(rings - 1, segment + 1)]);
    }
    Mesh::new(vertices, faces)
}

fn benchmarks(c: &mut Criterion) {
    let mesh = sphere(20.0, 320, 320);
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
        infill: Some(InfillSettings {
            pattern: InfillPattern::Hive,
            size_mm: 5.0,
            density: 0.15,
        }),
        ..empty.clone()
    };

    c.bench_function("hollow a 40 mm ball with a 2 mm wall", |b| {
        b.iter(|| hollow(std::hint::black_box(mesh), bvh, &empty, Cancel::never()));
    });

    c.bench_function("hollow a 40 mm ball and fill it with a gyroid", |b| {
        b.iter(|| hollow(std::hint::black_box(mesh), bvh, &filled, Cancel::never()));
    });

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

    // Half a million triangles is the size of a scanned bust, and what the step aimed at.
    let dense = sphere(20.0, 500, 500);
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
