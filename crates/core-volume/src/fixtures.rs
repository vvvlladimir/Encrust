use core_geometry::{Mesh, Scalar, Vec3, cuboid};

use crate::drain::inward;
use crate::{Boolean, combine_exact};

/// The box from `outer.0` to `outer.1` with each box of `cavities` sealed in it, wound inward
/// as a hollowed model carries its cavity.
pub(crate) fn sealed(outer: (Vec3, Vec3), cavities: &[(Vec3, Vec3)]) -> Mesh {
    let mut mesh = cuboid(outer.0, outer.1);
    for (low, high) in cavities {
        mesh.append(&inward(&cuboid(*low, *high)));
    }
    mesh
}

/// Two 30 mm cubes 30 mm apart along X, joined through their middles by a 4 mm square bar,
/// nudged off the lattice.
pub(crate) fn dumbbell() -> Mesh {
    let nudge = Vec3::new(0.017, 0.023, 0.031);
    let cube = |x: Scalar| {
        cuboid(
            Vec3::new(x, 0.0, 0.0) + nudge,
            Vec3::new(x + 30.0, 30.0, 30.0) + nudge,
        )
    };
    let bar = cuboid(
        Vec3::new(29.0, 13.0, 13.0) + nudge,
        Vec3::new(61.0, 17.0, 17.0) + nudge,
    );
    combine_exact(&[cube(0.0), bar, cube(60.0)], Boolean::Union)
        .expect("three overlapping boxes unite")
}
