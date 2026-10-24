//! Test bodies whose volume and topology are known in closed form.
//!
//! Shared by the integration tests and the benchmarks through `#[path]`, so every
//! consumer uses a subset and unused ones are expected.
#![allow(dead_code)]

use std::f32::consts::TAU;

use core_geometry::{Mesh, Scalar, Vec3};

/// Axis-aligned cube from the origin to `(size, size, size)`, wound outwards.
/// Volume `size^3`, 8 vertices, 12 faces, Euler characteristic 2.
pub fn cube(size: Scalar) -> Mesh {
    core_geometry::cuboid(Vec3::ZERO, Vec3::splat(size))
}

/// The same cube written the way STL stores it: three fresh vertices per triangle.
pub fn cube_unwelded(size: Scalar) -> Mesh {
    let welded = cube(size);
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

/// Cube with `count` faces wound the wrong way round.
pub fn cube_with_inverted_faces(size: Scalar, count: usize) -> Mesh {
    let mut mesh = cube(size);
    for face in mesh.faces.iter_mut().take(count) {
        face.swap(1, 2);
    }
    mesh
}

/// Cube with its top two faces missing, so it has a square boundary loop.
pub fn open_box(size: Scalar) -> Mesh {
    let mut mesh = cube(size);
    mesh.faces.drain(2..4);
    mesh
}

/// Corner tetrahedron with legs of length one. Volume 1/6.
pub fn tetrahedron() -> Mesh {
    Mesh::new(
        vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z],
        vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
    )
}

/// Regular octahedron with vertices on the axes at distance `radius`.
/// Volume `4/3 * radius^3`.
pub fn octahedron(radius: Scalar) -> Mesh {
    let r = radius;
    Mesh::new(
        vec![
            Vec3::new(r, 0.0, 0.0),
            Vec3::new(-r, 0.0, 0.0),
            Vec3::new(0.0, r, 0.0),
            Vec3::new(0.0, -r, 0.0),
            Vec3::new(0.0, 0.0, r),
            Vec3::new(0.0, 0.0, -r),
        ],
        vec![
            [0, 2, 4],
            [2, 1, 4],
            [1, 3, 4],
            [3, 0, 4],
            [2, 0, 5],
            [1, 2, 5],
            [3, 1, 5],
            [0, 3, 5],
        ],
    )
}

/// Closed sphere approximation with `rings` latitude bands and `segments` meridians.
/// Its volume approaches `4/3 * PI * radius^3` from below as the counts grow.
pub fn uv_sphere(radius: Scalar, rings: usize, segments: usize) -> Mesh {
    core_geometry::ball(Vec3::ZERO, radius, rings, segments)
}

/// Mobius strip: a manifold surface with a boundary and no consistent orientation.
pub fn mobius_strip(segments: usize) -> Mesh {
    assert!(segments >= 3, "a strip needs at least 3 segments");
    let (ring_radius, half_width): (Scalar, Scalar) = (2.0, 0.5);

    let mut vertices = Vec::with_capacity(segments * 2);
    for segment in 0..segments {
        let u = TAU * segment as Scalar / segments as Scalar;
        let (sin_u, cos_u) = u.sin_cos();
        let (sin_half, cos_half) = (u / 2.0).sin_cos();
        for side in [-half_width, half_width] {
            let radius = side.mul_add(cos_half, ring_radius);
            vertices.push(Vec3::new(radius * cos_u, radius * sin_u, side * sin_half));
        }
    }

    let mut faces = Vec::with_capacity(segments * 2);
    for segment in 0..segments {
        let a = (segment * 2) as u32;
        let b = a + 1;
        let next = ((segment + 1) % segments * 2) as u32;
        // Closing the loop joins the far end to the near one with the two sides swapped.
        // That half twist is what makes the strip non-orientable.
        let (c, d) = if segment + 1 == segments {
            (next, next + 1)
        } else {
            (next + 1, next)
        };
        faces.push([a, b, c]);
        faces.push([a, c, d]);
    }

    Mesh::new(vertices, faces)
}
