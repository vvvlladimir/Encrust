use core_geometry::{Mesh, Vec3, cuboid};

use crate::drain::inward;

/// The box from `outer.0` to `outer.1` with each box of `cavities` sealed in it, wound inward
/// as a hollowed model carries its cavity.
pub(crate) fn sealed(outer: (Vec3, Vec3), cavities: &[(Vec3, Vec3)]) -> Mesh {
    let mut mesh = cuboid(outer.0, outer.1);
    for (low, high) in cavities {
        mesh.append(&inward(&cuboid(*low, *high)));
    }
    mesh
}
