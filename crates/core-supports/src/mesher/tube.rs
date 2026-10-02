use core_geometry::{Mesh, Scalar, Vec3};

/// Rings closer together than this are the same ring, and the band between them would be
/// a strip of zero-area faces. Well under the pitch of any printable panel.
pub const RING_EPSILON_MM: Scalar = 1e-4;

/// One circle of a tube: where along the tube it sits, and how wide it is there.
#[derive(Debug, Clone, Copy)]
pub struct Ring {
    /// Distance from the top of the tube along its axis, millimetres. Negative above it,
    /// which is where a sphere tip reaches.
    pub along_mm: Scalar,
    pub radius_mm: Scalar,
}

/// A straight run of a support: an axis, the circles along it and what closes its top.
pub struct Tube {
    pub top: Vec3,
    pub direction: Vec3,
    pub rings: Vec<Ring>,
    /// Sides the ring polygon is drawn with, which is what makes a foot a cube or a
    /// prism rather than a cylinder.
    pub sides: usize,
    /// The tip cone's apex, for a tube that starts at a contact on the model.
    pub apex: Option<Vec3>,
}

/// A tube with nothing in it, for a strut of no length.
pub fn empty_tube(top: Vec3) -> Tube {
    Tube {
        top,
        direction: -Vec3::Z,
        rings: Vec::new(),
        sides: 3,
        apex: None,
    }
}

/// Rings of a ball whose top sits `bite_mm` above the contact, along the tube's axis.
///
/// The poles are left out: the cap vertex the sweep puts above the first ring is the top
/// pole, and the bottom one is inside the segment underneath.
pub fn ball(rings: &mut Vec<Ring>, bite_mm: Scalar, radius_mm: Scalar, steps: u32) {
    for step in 1..steps {
        let angle = std::f32::consts::PI * step as Scalar / steps as Scalar;
        push(
            rings,
            radius_mm.mul_add(1.0 - angle.cos(), -bite_mm),
            radius_mm * angle.sin(),
        );
    }
}

/// Adds a ring, never above the one before it and never twice in the same place.
pub fn push(rings: &mut Vec<Ring>, along_mm: Scalar, radius_mm: Scalar) {
    let along_mm = rings
        .last()
        .map_or(along_mm, |last: &Ring| along_mm.max(last.along_mm));
    let same = rings.last().is_some_and(|last: &Ring| {
        (last.along_mm - along_mm).abs() < RING_EPSILON_MM
            && (last.radius_mm - radius_mm).abs() < RING_EPSILON_MM
    });
    if !same {
        rings.push(Ring {
            along_mm,
            radius_mm,
        });
    }
}

/// The two unit vectors a tube's circles are drawn in, so that the polygon runs the same
/// way round whichever way the tube points.
///
/// Straight down is the common case and keeps the axes the plate itself is measured in.
pub fn frame(direction: Vec3) -> (Vec3, Vec3) {
    let across = if direction.z.abs() > 0.999 {
        Vec3::X
    } else {
        direction.cross(Vec3::Z).normalize()
    };
    (across, (-direction).cross(across))
}

/// Turns one tube into a closed solid: a cap on top, a band between each pair of rings,
/// and a flat disc underneath. Every edge ends up in exactly two faces.
pub fn sweep(mesh: &mut Mesh, tube: &Tube) {
    let Some(&bottom) = tube.rings.last() else {
        return;
    };
    if tube.rings.len() < 2 && tube.apex.is_none() {
        return;
    }

    let facets = tube.sides;
    let (across, up) = frame(tube.direction);
    let first = mesh.vertices.len() as u32;

    mesh.vertices.push(
        tube.apex
            .unwrap_or(tube.top + tube.direction * tube.rings[0].along_mm),
    );
    for ring in &tube.rings {
        let center = tube.top + tube.direction * ring.along_mm;
        for facet in 0..facets {
            let angle = std::f32::consts::TAU * facet as Scalar / facets as Scalar;
            mesh.vertices
                .push(center + (across * angle.cos() + up * angle.sin()) * ring.radius_mm);
        }
    }
    let bottom_center = mesh.vertices.len() as u32;
    mesh.vertices
        .push(tube.top + tube.direction * bottom.along_mm);

    let cap = first;
    let ring_at = |index: usize, facet: usize| -> u32 {
        first + 1 + (index * facets + facet % facets) as u32
    };
    for facet in 0..facets {
        mesh.faces
            .push([cap, ring_at(0, facet), ring_at(0, facet + 1)]);
    }

    for upper in 0..tube.rings.len().saturating_sub(1) {
        let lower = upper + 1;
        for facet in 0..facets {
            mesh.faces.push([
                ring_at(upper, facet),
                ring_at(lower, facet),
                ring_at(upper, facet + 1),
            ]);
            mesh.faces.push([
                ring_at(upper, facet + 1),
                ring_at(lower, facet),
                ring_at(lower, facet + 1),
            ]);
        }
    }

    let last = tube.rings.len() - 1;
    for facet in 0..facets {
        mesh.faces.push([
            bottom_center,
            ring_at(last, facet + 1),
            ring_at(last, facet),
        ]);
    }
}
