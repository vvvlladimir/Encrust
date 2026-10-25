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

/// How many rings a rounded end is drawn with, the tube's own last ring among them. At a
/// bend the outside of the joint is these ends, so they are drawn round; see ADR 0341.
pub const DOME_RINGS: u32 = 6;

/// A straight run of a support: an axis, the circles along it and what closes its top.
pub struct Tube {
    pub top: Vec3,
    pub direction: Vec3,
    pub rings: Vec<Ring>,
    /// Sides the ring polygon is drawn with, which is what makes a foot a cube or a
    /// prism rather than a cylinder.
    pub sides: usize,
    /// What closes the top: the tip cone's apex for a tube that starts at a contact, the
    /// pole of its dome for one that starts at a joint.
    pub apex: Option<Vec3>,
    /// The pole of a rounded bottom, for a tube that ends at a joint.
    pub tail: Option<Vec3>,
}

/// A tube with nothing in it, for a strut of no length.
pub fn empty_tube(top: Vec3) -> Tube {
    Tube {
        top,
        direction: -Vec3::Z,
        rings: Vec::new(),
        sides: 3,
        apex: None,
        tail: None,
    }
}

/// The rings of a dome a tube of `radius_mm` starts under, down to its full width at the
/// top of the tube. The pole, `radius_mm` back along the axis, is the tube's apex.
pub fn dome(rings: &mut Vec<Ring>, radius_mm: Scalar) {
    for step in 1..=DOME_RINGS {
        let angle = std::f32::consts::FRAC_PI_2 * step as Scalar / DOME_RINGS as Scalar;
        push(rings, -radius_mm * angle.cos(), radius_mm * angle.sin());
    }
}

/// The rings that round a tube of `radius_mm` off below `along_mm`, where its body ends.
/// The pole, `radius_mm` further along the axis, is the tube's tail.
pub fn round_off(rings: &mut Vec<Ring>, along_mm: Scalar, radius_mm: Scalar) {
    for step in 0..DOME_RINGS {
        let angle = std::f32::consts::FRAC_PI_2 * step as Scalar / DOME_RINGS as Scalar;
        push(
            rings,
            radius_mm.mul_add(angle.sin(), along_mm),
            radius_mm * angle.cos(),
        );
    }
}

/// Rings of a ball whose top sits `bite_mm` above the contact along the tube's axis; the poles
/// are left out: the sweep's cap is the top and the bottom is inside the segment under it.
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

/// The two unit vectors a tube's circles are drawn in, so the polygon runs the same way round
/// whichever way it points; straight down keeps the plate's own axes.
pub fn frame(direction: Vec3) -> (Vec3, Vec3) {
    let across = if direction.z.abs() > 0.999 {
        Vec3::X
    } else {
        direction.cross(Vec3::Z).normalize()
    };
    (across, (-direction).cross(across))
}

/// Turns one tube into a closed solid: a cap on top, a band between each pair of rings,
/// and a flat disc or a rounded end underneath. Every edge ends up in exactly two faces.
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
    let last = tube.rings.len() - 1;
    for ring in &tube.rings {
        let center = tube.top + tube.direction * ring.along_mm;
        for facet in 0..facets {
            let angle = std::f32::consts::TAU * facet as Scalar / facets as Scalar;
            let offset = (across * angle.cos() + up * angle.sin()) * ring.radius_mm;
            mesh.vertices.push(center + offset);
        }
    }
    let bottom_center = mesh.vertices.len() as u32;
    mesh.vertices.push(
        tube.tail
            .unwrap_or(tube.top + tube.direction * bottom.along_mm),
    );

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

    for facet in 0..facets {
        mesh.faces.push([
            bottom_center,
            ring_at(last, facet + 1),
            ring_at(last, facet),
        ]);
    }
}
