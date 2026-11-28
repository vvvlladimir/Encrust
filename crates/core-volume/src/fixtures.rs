use core_geometry::{Bvh, Mesh, Scalar, Transform, Vec3, cuboid};

use crate::drain::inward;
use crate::{Boolean, HoleSize, ModelHollow, combine_exact};

/// The box from `outer.0` to `outer.1` with each box of `cavities` sealed in it, wound inward
/// as a hollowed model carries its cavity.
pub(crate) fn sealed(outer: (Vec3, Vec3), cavities: &[(Vec3, Vec3)]) -> Mesh {
    let mut mesh = cuboid(outer.0, outer.1);
    for (low, high) in cavities {
        mesh.append(&inward(&cuboid(*low, *high)));
    }
    mesh
}

/// `parts` as one mesh, each keeping its own vertices.
pub(crate) fn joined(parts: &[Mesh]) -> Mesh {
    let mut whole = Mesh::default();
    for part in parts {
        whole.append(part);
    }
    whole
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

/// Drills into `hollow` the hole a click at `point` on the face of `normal` makes, on the model
/// `(mesh, bvh, transform)`; says whether it was new.
pub(crate) fn drained(
    hollow: &mut ModelHollow,
    (mesh, bvh, transform): (&Mesh, &Bvh, Transform),
    (point, normal): (Vec3, Vec3),
    size: HoleSize,
) -> bool {
    let hole = ModelHollow::drain_at(mesh, bvh, point, normal, size, transform);
    hollow.add_hole(hole.expect("the point drains"))
}
