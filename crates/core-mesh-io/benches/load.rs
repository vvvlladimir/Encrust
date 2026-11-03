//! Import throughput, on the critical path of every model opened in the window.
#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use std::f32::consts::PI;
use std::fmt::Write as _;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use core_geometry::{Mesh, Vec3};
use core_mesh_io::{MeshLoader, ObjLoader, StlLoader, ThreeMfLoader};
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

/// A 3MF package of one mesh object, deflated as exporters write it.
fn write_3mf(mesh: &Mesh, path: &Path) {
    let file = fs::File::create(path).expect("the temporary directory is writable");
    let mut zip = zip::ZipWriter::new(file);
    let deflated = zip::write::SimpleFileOptions::default();
    let mut part = |name: &str, body: &[u8]| {
        zip.start_file(name, deflated).expect("writing to a file");
        zip.write_all(body).expect("writing to a file");
    };
    part(
        "[Content_Types].xml",
        br#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="model" ContentType="application/vnd.ms-package.3dmanufacturing-3dmodel+xml"/></Types>"#,
    );
    part(
        "_rels/.rels",
        br#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Target="/3D/3dmodel.model" Id="rel0" Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel"/></Relationships>"#,
    );
    let mut model = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?><model unit="millimeter" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02"><resources><object id="1" type="model"><mesh><vertices>"#,
    );
    for vertex in &mesh.vertices {
        write!(
            model,
            r#"<vertex x="{}" y="{}" z="{}"/>"#,
            vertex.x, vertex.y, vertex.z
        )
        .expect("writing to a string");
    }
    model += "</vertices><triangles>";
    for [a, b, c] in &mesh.faces {
        write!(model, r#"<triangle v1="{a}" v2="{b}" v3="{c}"/>"#).expect("writing to a string");
    }
    model +=
        r#"</triangles></mesh></object></resources><build><item objectid="1"/></build></model>"#;
    part("3D/3dmodel.model", model.as_bytes());
    zip.finish().expect("writing to a file");
}

fn benchmarks(c: &mut Criterion) {
    let mesh = sphere(20.0, 320, 320);
    let directory = std::env::temp_dir().join("encrust-mesh-io-bench");
    fs::create_dir_all(&directory).expect("the temporary directory is writable");

    let stl: PathBuf = directory.join("sphere.stl");
    let obj: PathBuf = directory.join("sphere.obj");
    write_stl(&mesh, &stl);
    write_obj(&mesh, &obj);
    let three_mf: PathBuf = directory.join("sphere.3mf");
    write_3mf(&mesh, &three_mf);

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

    c.bench_function("load 200k triangles from 3mf", |b| {
        b.iter(|| {
            ThreeMfLoader
                .load(std::hint::black_box(&three_mf))
                .expect("the fixture loads")
        });
    });
}

criterion_group!(benches, benchmarks);
criterion_main!(benches);
