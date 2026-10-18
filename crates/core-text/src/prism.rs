use core_geometry::{Mesh, Scalar, Vec2, triangulate};

use crate::outline::Shape;

/// `shape` stood up from `bottom_mm` to `top_mm` along z: a closed mesh wound outward, its
/// holes cut right through.
pub fn prism(shape: &Shape, bottom_mm: Scalar, top_mm: Scalar) -> Mesh {
    let mut flat: Vec<Vec2> = shape.outer.clone();
    let mut rings: Vec<Vec<u32>> = vec![(0..shape.outer.len() as u32).collect()];
    for hole in &shape.holes {
        let start = flat.len() as u32;
        flat.extend_from_slice(hole);
        rings.push((start..flat.len() as u32).collect());
    }
    let holes: Vec<&Vec<u32>> = rings[1..].iter().collect();
    let mut cap = triangulate(&rings[0], &holes, &flat);
    // The outer ring runs anticlockwise, so the cap should face up; earcut does not promise it.
    let facing: Scalar = cap
        .iter()
        .map(|[a, b, c]| {
            let (a, b, c) = (flat[*a as usize], flat[*b as usize], flat[*c as usize]);
            (b - a).perp_dot(c - a)
        })
        .sum();
    if facing < 0.0 {
        for triangle in &mut cap {
            triangle.swap(1, 2);
        }
    }

    let count = flat.len() as u32;
    let mut vertices: Vec<_> = flat.iter().map(|point| point.extend(bottom_mm)).collect();
    vertices.extend(flat.iter().map(|point| point.extend(top_mm)));
    let mut faces = Vec::with_capacity(2 * cap.len() + 2 * flat.len());
    for [a, b, c] in &cap {
        faces.push([*a + count, *b + count, *c + count]);
        faces.push([*a, *c, *b]);
    }
    for ring in &rings {
        for (index, a) in ring.iter().enumerate() {
            let b = ring[(index + 1) % ring.len()];
            faces.push([*a, b, b + count]);
            faces.push([*a, b + count, *a + count]);
        }
    }
    Mesh::new(vertices, faces)
}
