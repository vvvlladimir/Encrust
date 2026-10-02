//! Import throughput, on the critical path of every model opened in the window.
#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use std::f32::consts::PI;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use core_geometry::{Mesh, Vec3};
use core_mesh_io::{MeshLoader, ObjLoader, StlLoader};
use criterion::{Criterion, criterion_group, criterion_main};

/// Roughly 200k triangles, the size of a detailed miniature.
fn sphere(radius: f32, segments: usize, rings: usize) -> Mesh {
    let mut vertices = Vec::with_capacity((rings + 1) * (segments + 1));
    for ring in 0..=rings {
        let phi = PI * ring as f32 / rings as f32;
        for segment in 0..=segments {
            let theta = 2.0 * PI * segment as f32 / segments as f32;
            vertices.push(Vec3::new(
                radius * phi.sin() * theta.cos(),
                radius * phi.sin() * theta.sin(),
                radius * phi.cos(),
            ));
        }
    }

    let mut faces = Vec::with_capacity(rings * segments * 2);
    let stride = segments + 1;
    for ring in 0..rings {
        for segment in 0..segments {
            let a = (ring * stride + segment) as u32;
            let b = a + stride as u32;
            faces.push([a, b, a + 1]);
            faces.push([a + 1, b, b + 1]);
        }
    }
    Mesh::new(vertices, faces)
}

/// Binary STL as an exporter writes it: no shared vertices, a zero normal per face.
fn write_stl(mesh: &Mesh, path: &Path) {
    let mut bytes = Vec::with_capacity(84 + mesh.faces.len() * 50);
    bytes.extend_from_slice(&[0u8; 80]);
    bytes.extend_from_slice(&(mesh.faces.len() as u32).to_le_bytes());
    for face in &mesh.faces {
        bytes.extend_from_slice(&[0u8; 12]);
        for index in face {
            let vertex = mesh.vertices[*index as usize];
            for component in [vertex.x, vertex.y, vertex.z] {
                bytes.extend_from_slice(&component.to_le_bytes());
            }
        }
        bytes.extend_from_slice(&[0u8; 2]);
    }
    fs::write(path, bytes).expect("the temporary directory is writable");
}

fn write_obj(mesh: &Mesh, path: &Path) {
    let file = fs::File::create(path).expect("the temporary directory is writable");
    let mut out = std::io::BufWriter::new(file);
    for vertex in &mesh.vertices {
        writeln!(out, "v {} {} {}", vertex.x, vertex.y, vertex.z).expect("writing to a file");
    }
    for face in &mesh.faces {
        writeln!(out, "f {} {} {}", face[0] + 1, face[1] + 1, face[2] + 1)
            .expect("writing to a file");
    }
}

fn benchmarks(c: &mut Criterion) {
    let mesh = sphere(20.0, 320, 320);
    let directory = std::env::temp_dir().join("encrust-mesh-io-bench");
    fs::create_dir_all(&directory).expect("the temporary directory is writable");

    let stl: PathBuf = directory.join("sphere.stl");
    let obj: PathBuf = directory.join("sphere.obj");
    write_stl(&mesh, &stl);
    write_obj(&mesh, &obj);

    c.bench_function("load 200k triangles from binary stl", |b| {
        b.iter(|| {
            StlLoader
                .load(std::hint::black_box(&stl))
                .expect("the fixture loads")
        });
    });

    c.bench_function("load 200k triangles from obj", |b| {
        b.iter(|| {
            ObjLoader
                .load(std::hint::black_box(&obj))
                .expect("the fixture loads")
        });
    });
}

criterion_group!(benches, benchmarks);
criterion_main!(benches);
