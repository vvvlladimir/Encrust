use core_geometry::{Mesh, Scalar, Vec2, extrude};

use crate::outline::Shape;
use crate::sheet::Sheet;

/// `shape` stood up from `bottom_mm` to `top_mm` along z: a closed mesh wound outward, its
/// holes cut right through.
pub fn prism(shape: &Shape, bottom_mm: Scalar, top_mm: Scalar) -> Mesh {
    let sheet = Sheet::new(shape);
    let count = sheet.flat.len();
    extrude(
        &sheet.flat,
        &sheet.cap,
        Vec2::extend,
        &vec![bottom_mm; count],
        &vec![top_mm; count],
    )
}
