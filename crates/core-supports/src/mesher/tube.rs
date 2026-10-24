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
/// bend the outside of the joint is these ends, so they are drawn round; see ADR 0343.
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
    /// The outward normal of the face a plane tip sits on: its first ring is laid along
    /// the axis onto that face, so the whole disc meets it however the head leans.
    pub seat: Option<Vec3>,
    /// The same for the face a tube standing on the model ends on, laying its last ring.
    pub sole: Option<Vec3>,
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
        seat: None,
        sole: None,
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

/// The least a head may lean off the face its tip sits on, as the cosine between its
/// axis and the face's normal, for the disc to be laid onto that face: past it the disc
/// would stretch along the face further than the head is wide.
const SEAT_MIN_COS: Scalar = 0.25;

/// How far along the axis each point of ring `index` moves to lie on the face `normal`
/// names, given the point's offset from the axis; no further than halfway to the ring
/// beside it, so the band between the two never turns over.
fn laid(tube: &Tube, index: usize, normal: Option<Vec3>) -> impl Fn(Vec3) -> Scalar {
    let rings = &tube.rings;
    let beside = if index == 0 {
        1
    } else {
        index.saturating_sub(1)
    };
    let room_mm = match (rings.get(index), rings.get(beside)) {
        (Some(ring), Some(other)) if beside != index => {
            (ring.along_mm - other.along_mm).abs() / 2.0
        }
        _ => 0.0,
    };
    let direction = tube.direction;
    let face = normal
        .map(|normal| (normal, direction.dot(normal)))
        .filter(|(_, cos)| cos.abs() >= SEAT_MIN_COS);
    move |offset: Vec3| {
        face.map_or(0.0, |(normal, cos)| {
            (-offset.dot(normal) / cos).clamp(-room_mm, room_mm)
        })
    }
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
    let (seated, soled) = (laid(tube, 0, tube.seat), laid(tube, last, tube.sole));
    for (index, ring) in tube.rings.iter().enumerate() {
        let center = tube.top + tube.direction * ring.along_mm;
        for facet in 0..facets {
            let angle = std::f32::consts::TAU * facet as Scalar / facets as Scalar;
            let offset = (across * angle.cos() + up * angle.sin()) * ring.radius_mm;
            let along = match index {
                0 => seated(offset),
                at if at == last => soled(offset),
                _ => 0.0,
            };
            mesh.vertices.push(center + offset + tube.direction * along);
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

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::diagnose;

    /// A tip leaving a face looking straight down at `lean_deg` off vertical, its disc
    /// laid on that face.
    fn leaning_tip(lean_deg: Scalar) -> Mesh {
        let lean = lean_deg.to_radians();
        let mut mesh = Mesh::default();
        sweep(
            &mut mesh,
            &Tube {
                top: Vec3::new(0.0, 0.0, 10.0),
                direction: Vec3::new(lean.sin(), 0.0, -lean.cos()),
                rings: vec![
                    Ring {
                        along_mm: 0.0,
                        radius_mm: 0.12,
                    },
                    Ring {
                        along_mm: 2.0,
                        radius_mm: 0.6,
                    },
                ],
                sides: 12,
                apex: None,
                tail: None,
                seat: Some(-Vec3::Z),
                sole: None,
            },
        );
        mesh
    }

    #[test]
    fn a_leaning_tip_meets_its_face_with_the_whole_disc() {
        let mesh = leaning_tip(45.0);
        // The cap's centre and the first ring: one vertex, then twelve.
        for vertex in &mesh.vertices[..13] {
            assert!(
                (vertex.z - 10.0).abs() < 1e-5,
                "the face the tip sits on is z = 10, got {vertex}"
            );
        }
        assert_eq!(diagnose(&mesh).boundary_edges, 0, "the tip is still closed");
    }

    #[test]
    fn a_leaning_trunk_stands_on_its_face_with_the_whole_disc() {
        let lean = 45f32.to_radians();
        let mut mesh = Mesh::default();
        let direction = Vec3::new(lean.sin(), 0.0, -lean.cos());
        let top = Vec3::new(0.0, 0.0, 10.0);
        sweep(
            &mut mesh,
            &Tube {
                top,
                direction,
                rings: vec![
                    Ring {
                        along_mm: 0.0,
                        radius_mm: 0.6,
                    },
                    Ring {
                        along_mm: 2.0,
                        radius_mm: 0.12,
                    },
                ],
                sides: 12,
                apex: None,
                tail: None,
                seat: None,
                sole: Some(Vec3::Z),
            },
        );
        let floor_z = (top + direction * 2.0).z;
        // The cap's centre, the first ring, then the last ring of twelve.
        for vertex in &mesh.vertices[13..25] {
            assert!(
                (vertex.z - floor_z).abs() < 1e-5,
                "the face the trunk stands on is z = {floor_z}, got {vertex}"
            );
        }
        assert_eq!(
            diagnose(&mesh).boundary_edges,
            0,
            "the trunk is still closed"
        );
    }

    #[test]
    fn a_tip_leaning_along_its_face_keeps_its_disc_square() {
        let mesh = leaning_tip(80.0);
        let lowest = mesh.vertices[1..13]
            .iter()
            .map(|vertex| vertex.z)
            .fold(Scalar::INFINITY, Scalar::min);
        assert!(
            lowest < 10.0 - 1e-3,
            "past the seat's angle the disc stays across the axis, got {lowest}"
        );
    }
}
