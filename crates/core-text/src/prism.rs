use core_geometry::{Mesh, Scalar};

use crate::outline::Shape;
use crate::sheet::Sheet;

/// `shape` stood up from `bottom_mm` to `top_mm` along z: a closed mesh wound outward, its
/// holes cut right through.
pub fn prism(shape: &Shape, bottom_mm: Scalar, top_mm: Scalar) -> Mesh {
    let sheet = Sheet::new(shape);
    let count = sheet.flat.len();
    stand(&sheet, &vec![bottom_mm; count], &vec![top_mm; count])
}

/// `sheet` stood up along z, each of its points from its own bottom to its own top, in
/// millimetres: closed and wound outward wherever every top is above its bottom.
pub(crate) fn stand(sheet: &Sheet, bottom_mm: &[Scalar], top_mm: &[Scalar]) -> Mesh {
    let flat = &sheet.flat;
    let count = flat.len() as u32;
    let mut vertices: Vec<_> = flat
        .iter()
        .zip(bottom_mm)
        .map(|(point, z)| point.extend(*z))
        .collect();
    vertices.extend(flat.iter().zip(top_mm).map(|(point, z)| point.extend(*z)));
    let walls: usize = sheet.rings.iter().map(Vec::len).sum();
    let mut faces = Vec::with_capacity(2 * sheet.cap.len() + 2 * walls);
    for [a, b, c] in &sheet.cap {
        faces.push([*a + count, *b + count, *c + count]);
        faces.push([*a, *c, *b]);
    }
    for ring in &sheet.rings {
        for (index, a) in ring.iter().enumerate() {
            let b = ring[(index + 1) % ring.len()];
            faces.push([*a, b, b + count]);
            faces.push([*a, b + count, *a + count]);
        }
    }
    Mesh::new(vertices, faces)
}
