#[path = "../tests/shared/bodies.rs"]
mod bodies;

use core_geometry::{
    DEFAULT_WELD_TOLERANCE, Mesh, diagnose, orient_outward, remove_duplicate_faces, split, weld,
};
use criterion::{Criterion, criterion_group, criterion_main};

/// Roughly 200k triangles, the size of a detailed miniature.
fn sample() -> Mesh {
    bodies::uv_sphere(20.0, 320, 320)
}

/// Roughly 2M triangles, a dense sculpture: where the checks run on every core.
fn dense_sample() -> Mesh {
    bodies::uv_sphere(20.0, 1000, 1000)
}

/// The same mesh as an STL exporter would write it: no shared vertices at all.
fn unwelded_sample() -> Mesh {
    let welded = sample();
    let mut vertices = Vec::with_capacity(welded.faces.len() * 3);
    let mut faces = Vec::with_capacity(welded.faces.len());
    for face in &welded.faces {
        let base = vertices.len() as u32;
        for index in face {
            vertices.push(welded.vertices[*index as usize]);
        }
        faces.push([base, base + 1, base + 2]);
    }
    Mesh::new(vertices, faces)
}

fn benchmarks(c: &mut Criterion) {
    let raw = unwelded_sample();
    let clean = sample();

    c.bench_function("weld 200k triangles", |b| {
        b.iter(|| weld(std::hint::black_box(&raw), DEFAULT_WELD_TOLERANCE));
    });

    c.bench_function("diagnose 200k triangles", |b| {
        b.iter(|| diagnose(std::hint::black_box(&clean)));
    });

    c.bench_function("orient 200k triangles", |b| {
        b.iter_batched(
            || clean.clone(),
            |mut mesh| orient_outward(&mut mesh),
            criterion::BatchSize::LargeInput,
        );
    });

    c.bench_function("remove duplicates from 200k triangles", |b| {
        b.iter_batched(
            || clean.clone(),
            |mut mesh| remove_duplicate_faces(&mut mesh),
            criterion::BatchSize::LargeInput,
        );
    });

    c.bench_function("split 200k triangles", |b| {
        b.iter(|| split(std::hint::black_box(&clean)));
    });

    let dense = dense_sample();
    let mut group = c.benchmark_group("2M triangles");
    group.sample_size(20);
    group.bench_function("diagnose", |b| {
        b.iter(|| diagnose(std::hint::black_box(&dense)));
    });
    group.bench_function("orient", |b| {
        b.iter_batched(
            || dense.clone(),
            |mut mesh| orient_outward(&mut mesh),
            criterion::BatchSize::LargeInput,
        );
    });
    group.bench_function("split", |b| {
        b.iter(|| split(std::hint::black_box(&dense)));
    });
    group.finish();
}

criterion_group!(benches, benchmarks);
criterion_main!(benches);
