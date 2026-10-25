use crate::{DEFAULT_WELD_TOLERANCE, Mesh, Scalar, Vec3, cuboid, weld};

/// The unit cube with its top cut into `n` by `n` squares whose corners the sides do not
/// share: every rim edge of the sides has `n - 1` vertices of the top standing on it.
pub(crate) fn cube_with_a_finer_top(n: u32, lift_mm: Scalar) -> Mesh {
    let mut mesh = cuboid(Vec3::ZERO, Vec3::ONE);
    let vertices = mesh.vertices.clone();
    mesh.faces
        .retain(|face| face.iter().any(|&corner| vertices[corner as usize].z < 0.5));
    let first = mesh.vertices.len() as u32;
    let at = |i: u32, j: u32| first + j * (n + 1) + i;
    for j in 0..=n {
        for i in 0..=n {
            let rim = i == 0 || j == 0 || i == n || j == n;
            let corner = (i == 0 || i == n) && (j == 0 || j == n);
            let outward = if rim && !corner { lift_mm } else { 0.0 };
            mesh.vertices.push(Vec3::new(
                i as Scalar / n as Scalar,
                j as Scalar / n as Scalar,
                1.0 + outward,
            ));
        }
    }
    for j in 0..n {
        for i in 0..n {
            mesh.faces.push([at(i, j), at(i + 1, j), at(i + 1, j + 1)]);
            mesh.faces.push([at(i, j), at(i + 1, j + 1), at(i, j + 1)]);
        }
    }
    weld(&mesh, DEFAULT_WELD_TOLERANCE).mesh
}

/// A 10 mm cube with an open triangle of 0.5 mm² floating beside it.
pub(crate) fn cube_and_a_speck() -> Mesh {
    let mut mesh = cuboid(Vec3::ZERO, Vec3::splat(10.0));
    let first = mesh.vertices.len() as u32;
    mesh.vertices.extend([
        Vec3::new(20.0, 0.0, 0.0),
        Vec3::new(21.0, 0.0, 0.0),
        Vec3::new(20.0, 1.0, 0.0),
    ]);
    mesh.faces.push([first, first + 1, first + 2]);
    mesh
}
