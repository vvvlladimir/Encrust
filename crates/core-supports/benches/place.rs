#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use core_geometry::{Mesh, Scalar, Vec3};
use core_slicer::{PlaneSliceEngine, SliceEngine, SliceSettings, Sliced};
use core_supports::generate_supports;
use criterion::{Criterion, criterion_group, criterion_main};
use printer_profiles::SupportProfile;

/// The layer height the shipped resin profile prints at.
const LAYER_HEIGHT_MM: Scalar = 0.05;

/// An axis-aligned box spanning `min`..`max`, wound outwards.
fn box_mesh(min: Vec3, max: Vec3) -> Mesh {
    let vertices = vec![
        Vec3::new(min.x, min.y, min.z),
        Vec3::new(max.x, min.y, min.z),
        Vec3::new(max.x, max.y, min.z),
        Vec3::new(min.x, max.y, min.z),
        Vec3::new(min.x, min.y, max.z),
        Vec3::new(max.x, min.y, max.z),
        Vec3::new(max.x, max.y, max.z),
        Vec3::new(min.x, max.y, max.z),
    ];
    let faces = vec![
        [0, 2, 1],
        [0, 3, 2],
        [4, 5, 6],
        [4, 6, 7],
        [0, 1, 5],
        [0, 5, 4],
        [1, 2, 6],
        [1, 6, 5],
        [2, 3, 7],
        [2, 7, 6],
        [3, 0, 4],
        [3, 4, 7],
    ];
    Mesh::new(vertices, faces)
}

fn merge(meshes: &[Mesh]) -> Mesh {
    let mut merged = Mesh::default();
    for mesh in meshes {
        let offset = merged.vertices.len() as u32;
        merged.vertices.extend_from_slice(&mesh.vertices);
        merged.faces.extend(
            mesh.faces
                .iter()
                .map(|[a, b, c]| [a + offset, b + offset, c + offset]),
        );
    }
    merged
}

/// Four legs under a 40 mm table, with a second table over it: 1200 layers, of which a
/// handful carry islands and coasts and the rest are walls standing on themselves.
fn tables() -> Sliced {
    let mut parts = Vec::new();
    for (x, y) in [(5.0, 5.0), (31.0, 5.0), (5.0, 31.0), (31.0, 31.0)] {
        parts.push(box_mesh(
            Vec3::new(x, y, 0.0),
            Vec3::new(x + 4.0, y + 4.0, 20.0),
        ));
    }
    parts.push(box_mesh(
        Vec3::ZERO.with_z(20.0),
        Vec3::new(40.0, 40.0, 24.0),
    ));
    parts.push(box_mesh(
        Vec3::new(10.0, 10.0, 50.0),
        Vec3::new(30.0, 30.0, 60.0),
    ));

    PlaneSliceEngine
        .slice(
            &merge(&parts),
            &SliceSettings {
                layer_height: LAYER_HEIGHT_MM,
                ..SliceSettings::default()
            },
        )
        .expect("closed boxes slice")
}

/// A ball resting on the plate: 600 layers, every one of them under its equator hanging
/// over nothing, which is the case that actually puts supports down.
fn ball(segments: u32, rings: u32) -> Sliced {
    let radius = 15.0;
    let mut vertices = Vec::new();
    let mut faces = Vec::new();
    for ring in 0..=rings {
        let phi = std::f32::consts::PI * ring as Scalar / rings as Scalar;
        for segment in 0..segments {
            let theta = std::f32::consts::TAU * segment as Scalar / segments as Scalar;
            vertices.push(Vec3::new(
                radius * phi.sin() * theta.cos(),
                radius * phi.sin() * theta.sin(),
                radius - radius * phi.cos(),
            ));
        }
    }
    for ring in 0..rings {
        for segment in 0..segments {
            let next = (segment + 1) % segments;
            let low_left = ring * segments + segment;
            let low_right = ring * segments + next;
            let high_left = (ring + 1) * segments + segment;
            let high_right = (ring + 1) * segments + next;
            faces.push([low_left, high_right, high_left]);
            faces.push([low_left, low_right, high_right]);
        }
    }

    PlaneSliceEngine
        .slice(
            &Mesh::new(vertices, faces),
            &SliceSettings {
                layer_height: LAYER_HEIGHT_MM,
                ..SliceSettings::default()
            },
        )
        .expect("a closed ball slices")
}

fn placing(c: &mut Criterion) {
    let profile = SupportProfile::medium();

    let tables = tables();
    c.bench_function("generate_supports/tables, 1200 layers", |b| {
        b.iter(|| generate_supports(&tables, LAYER_HEIGHT_MM, &profile, &[], None, &mut |_| true));
    });

    let coarse = ball(256, 128);
    c.bench_function("generate_supports/ball, 600 layers", |b| {
        b.iter(|| generate_supports(&coarse, LAYER_HEIGHT_MM, &profile, &[], None, &mut |_| true));
    });

    // The same ball with sixteen times the faces, and so sixteen times the points on
    // every contour: what a scanned or sculpted model arrives as. What it costs to place
    // supports must follow the shape, not the triangle count.
    let dense = ball(1024, 512);
    c.bench_function("generate_supports/ball, 1 M faces", |b| {
        b.iter(|| generate_supports(&dense, LAYER_HEIGHT_MM, &profile, &[], None, &mut |_| true));
    });
}

criterion_group!(benches, placing);
criterion_main!(benches);
