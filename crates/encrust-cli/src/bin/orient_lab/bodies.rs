//! Bodies whose right orientation is known before anything is measured, so the lab has a
//! corpus without a file on disk.

use core_geometry::{Mesh, Vec3};

/// Every built-in body, by the name the table prints.
pub fn all() -> Vec<(&'static str, Mesh)> {
    vec![
        (
            "slab-40x40x4",
            cuboid(Vec3::ZERO, Vec3::new(40.0, 40.0, 4.0)),
        ),
        (
            "column-6x6x80",
            cuboid(Vec3::ZERO, Vec3::new(6.0, 6.0, 80.0)),
        ),
        ("cup-30x30x20", cup()),
        ("mushroom", mushroom()),
        ("pyramid-point-down", pyramid_point_down()),
        ("figure-on-base", figure_on_base()),
        ("pin-plate", pin_plate()),
        ("bracket", bracket()),
    ]
}

/// Axis-aligned box from `min` to `max`, twelve triangles wound outward.
fn cuboid(min: Vec3, max: Vec3) -> Mesh {
    core_geometry::cuboid(min, max)
}

/// Bodies laid into one mesh. They may overlap: the stack is filled by winding, so an
/// overlap cures once (ADR 0206).
fn merged(parts: &[Mesh]) -> Mesh {
    let mut mesh = Mesh::default();
    for part in parts {
        let offset = mesh.vertices.len() as u32;
        mesh.vertices.extend_from_slice(&part.vertices);
        mesh.faces.extend(
            part.faces
                .iter()
                .map(|[a, b, c]| [a + offset, b + offset, c + offset]),
        );
    }
    mesh
}

/// An open-topped box with 2 mm walls: open side up, its own floor seals the inside from
/// below, which is a suction cup.
fn cup() -> Mesh {
    let wall = 2.0;
    merged(&[
        cuboid(Vec3::ZERO, Vec3::new(30.0, 30.0, wall)),
        cuboid(Vec3::ZERO, Vec3::new(wall, 30.0, 20.0)),
        cuboid(
            Vec3::new(30.0 - wall, 0.0, 0.0),
            Vec3::new(30.0, 30.0, 20.0),
        ),
        cuboid(Vec3::ZERO, Vec3::new(30.0, wall, 20.0)),
        cuboid(
            Vec3::new(0.0, 30.0 - wall, 0.0),
            Vec3::new(30.0, 30.0, 20.0),
        ),
    ])
}

/// A 4 mm stem under a 20 by 20 cap: the cap's underside needs holding up however it
/// stands, unless it stands on the cap.
fn mushroom() -> Mesh {
    merged(&[
        cuboid(Vec3::new(8.0, 8.0, 0.0), Vec3::new(12.0, 12.0, 15.0)),
        cuboid(Vec3::new(0.0, 0.0, 15.0), Vec3::new(20.0, 20.0, 18.0)),
    ])
}

/// A torso with arms out and a head, on a wide base: a figure, whose base is the one face
/// nobody looks at and whose head is the last place a support should touch.
fn figure_on_base() -> Mesh {
    merged(&[
        cuboid(Vec3::ZERO, Vec3::new(30.0, 30.0, 3.0)),
        cuboid(Vec3::new(10.0, 10.0, 3.0), Vec3::new(20.0, 20.0, 30.0)),
        cuboid(Vec3::new(0.0, 13.0, 22.0), Vec3::new(30.0, 17.0, 26.0)),
        cuboid(Vec3::new(9.0, 9.0, 30.0), Vec3::new(21.0, 21.0, 40.0)),
    ])
}

/// Five 1.5 mm pins standing on a plate: a test piece drawn to print as it stands, where
/// any turn hangs every pin off its side.
fn pin_plate() -> Mesh {
    let mut parts = vec![cuboid(Vec3::ZERO, Vec3::new(40.0, 20.0, 2.0))];
    parts.extend((0..5).map(|pin| {
        let x = 4.0 + 8.0 * pin as f32;
        cuboid(Vec3::new(x, 9.0, 2.0), Vec3::new(x + 1.5, 10.5, 12.0))
    }));
    merged(&parts)
}

/// Two 3 mm slabs meeting at a right angle: lying on either, the other is a wall, and
/// stood on its end both are.
fn bracket() -> Mesh {
    merged(&[
        cuboid(Vec3::ZERO, Vec3::new(40.0, 30.0, 3.0)),
        cuboid(Vec3::ZERO, Vec3::new(3.0, 30.0, 40.0)),
    ])
}

/// A square pyramid standing on its apex: a point with no area under it that still needs a
/// support, and a wide base that is easy to stand on.
fn pyramid_point_down() -> Mesh {
    let vertices = vec![
        Vec3::new(10.0, 10.0, 0.0),
        Vec3::new(0.0, 0.0, 20.0),
        Vec3::new(20.0, 0.0, 20.0),
        Vec3::new(20.0, 20.0, 20.0),
        Vec3::new(0.0, 20.0, 20.0),
    ];
    let faces = vec![
        [0, 2, 1],
        [0, 3, 2],
        [0, 4, 3],
        [0, 1, 4],
        [1, 2, 3],
        [1, 3, 4],
    ];
    Mesh::new(vertices, faces)
}
