use std::f32::consts::{FRAC_PI_2, TAU};

use core_geometry::{Mesh, Quat, Scalar, Vec3};

use super::{Channel, Section};

/// The sharpest turn a channel takes at one point, radians. A mitre's reach past the tube
/// grows as the secant of half the turn, so a sharper one is split in two at each side of
/// the point, each a turn this size at most.
const SHARPEST_TURN: Scalar = FRAC_PI_2;

/// The least a split turn's two halves are set apart by, as a share of the turn's cosine,
/// so a channel doubling straight back on itself still has a leg between them.
const LEAST_COSINE: Scalar = 0.1;

/// One straight leg of a channel as it is cut.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Leg {
    pub from: Vec3,
    pub to: Vec3,
    /// The unit offset from the axis to the outline's first corner, square to the leg.
    pub corner: Vec3,
    /// The unit normal, along the channel, of the plane the leg is mitred off at where it
    /// meets the leg before or after it; `None` at an open end of the channel.
    pub start: Option<Vec3>,
    pub end: Option<Vec3>,
}

impl Channel {
    /// The legs this channel is cut as, its outline carried from one to the next by the
    /// least turn that takes the one leg onto the other, so a polygon never twists about the
    /// channel and its mitres meet corner to corner. See `docs/design/hollowing.md`.
    pub fn legs(&self) -> Vec<Leg> {
        let spine = spine(&self.points, self.diameter_mm / 2.0);
        let tangents: Vec<Vec3> = spine
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).normalize())
            .collect();
        let Some(first) = tangents.first() else {
            return Vec::new();
        };
        let section = Section::of(self.shape, self.turn_deg);
        let mut corner = section.corners(*first).next().unwrap_or(Vec3::ZERO);
        let mitre = |before: Vec3, after: Vec3| (before + after).try_normalize();
        let mut legs = Vec::with_capacity(tangents.len());
        for (index, tangent) in tangents.iter().enumerate() {
            if index > 0 {
                corner = Quat::from_rotation_arc(tangents[index - 1], *tangent) * corner;
            }
            legs.push(Leg {
                from: spine[index],
                to: spine[index + 1],
                corner,
                start: index
                    .checked_sub(1)
                    .and_then(|before| mitre(tangents[before], *tangent)),
                end: tangents
                    .get(index + 1)
                    .and_then(|after| mitre(*tangent, *after)),
            });
        }
        legs
    }
}

/// `points` with the repeats dropped and every turn sharper than [`SHARPEST_TURN`] split
/// into two, set apart far enough for a tube of `radius` to make each without folding.
fn spine(points: &[Vec3], radius: Scalar) -> Vec<Vec3> {
    let mut kept: Vec<Vec3> = Vec::with_capacity(points.len());
    for point in points {
        if kept.last().is_none_or(|last| last.distance(*point) > 1e-6) {
            kept.push(*point);
        }
    }
    let mut spine = Vec::with_capacity(kept.len() * 2);
    for (index, point) in kept.iter().enumerate() {
        let (Some(before), Some(after)) = (index.checked_sub(1), kept.get(index + 1)) else {
            spine.push(*point);
            continue;
        };
        let (into, out) = (*point - kept[before], *after - *point);
        let turn = into.angle_between(out);
        if turn <= SHARPEST_TURN {
            spine.push(*point);
            continue;
        }
        // Each half turns by half as much, and the leg between them is as long as the two
        // mitres reach in along it.
        let apart = radius * (turn / 4.0).tan() / (turn / 2.0).cos().max(LEAST_COSINE);
        let apart = apart.min(into.length() / 2.0).min(out.length() / 2.0);
        spine.push(*point - into.normalize() * apart);
        spine.push(*point + out.normalize() * apart);
    }
    spine
}

/// The closed prism `channel` digs, wound outward: one ring of its outline at each end and
/// at every mitre, so the outline holds all the way through.
pub(super) fn tunnel(channel: &Channel) -> Option<Mesh> {
    let radius = channel.diameter_mm / 2.0;
    let legs = channel.legs();
    if radius <= 0.0 || !radius.is_finite() || legs.is_empty() {
        return None;
    }
    let sides = Section::of(channel.shape, channel.turn_deg).sides;
    let ring = |leg: &Leg| -> Vec<Vec3> {
        let axis = (leg.to - leg.from).normalize();
        let across = axis.cross(leg.corner);
        (0..sides)
            .map(|side| {
                let (sin, cos) = (TAU * side as Scalar / sides as Scalar).sin_cos();
                (leg.corner * cos + across * sin) * radius
            })
            .collect()
    };

    let mut rings: Vec<Vec<Vec3>> = Vec::with_capacity(legs.len() + 1);
    rings.push(
        ring(&legs[0])
            .iter()
            .map(|offset| legs[0].from + *offset)
            .collect(),
    );
    for leg in &legs {
        let axis = (leg.to - leg.from).normalize();
        let at_end = |offset: &Vec3| match leg.end {
            // Carried along the leg onto the mitre plane through its end.
            Some(mitre) => leg.to + *offset - axis * (offset.dot(mitre) / axis.dot(mitre)),
            None => leg.to + *offset,
        };
        rings.push(ring(leg).iter().map(at_end).collect());
    }
    Some(prism(&rings, legs[0].from, legs[legs.len() - 1].to))
}

/// The rings joined side to side and closed by a fan to `first` and `last` at either end.
fn prism(rings: &[Vec<Vec3>], first: Vec3, last: Vec3) -> Mesh {
    let sides = rings[0].len() as u32;
    let mut vertices: Vec<Vec3> = rings.iter().flatten().copied().collect();
    let (low_cap, high_cap) = (vertices.len() as u32, vertices.len() as u32 + 1);
    vertices.push(first);
    vertices.push(last);

    let at = |ring: usize, side: u32| ring as u32 * sides + side % sides;
    let mut faces = Vec::with_capacity(2 * sides as usize * (rings.len() + 1));
    for ring in 0..rings.len() - 1 {
        for side in 0..sides {
            let (low, high) = (at(ring, side), at(ring + 1, side));
            let (low_next, high_next) = (at(ring, side + 1), at(ring + 1, side + 1));
            faces.push([low, low_next, high_next]);
            faces.push([low, high_next, high]);
        }
    }
    let end = rings.len() - 1;
    for side in 0..sides {
        faces.push([low_cap, at(0, side + 1), at(0, side)]);
        faces.push([high_cap, at(end, side), at(end, side + 1)]);
    }
    Mesh::new(vertices, faces)
}

#[cfg(test)]
mod tests {
    use core_geometry::{diagnose, signed_volume};

    use super::*;
    use crate::drain::HoleShape;

    fn square(points: Vec<Vec3>) -> Channel {
        Channel {
            points,
            diameter_mm: 2.0,
            shape: HoleShape::Polygon { sides: 4 },
            turn_deg: 0.0,
        }
    }

    #[test]
    fn a_bent_square_channel_is_one_closed_prism_holding_its_section_along_its_length() {
        let bent = square(vec![
            Vec3::ZERO,
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(10.0, 8.0, 0.0),
        ]);
        let dug = tunnel(&bent).expect("a bent channel digs");
        let report = diagnose(&dug);
        assert!(
            report.is_sound() && report.unbalanced_edges == 0 && report.shells == 1,
            "{report:?}"
        );
        // A square with its corners on 1 mm holds 2 mm2; a mitred bend of a section centred on
        // its axis gives up on the inside what it gains outside, so 2 mm2 by 18 mm.
        let volume = signed_volume(&dug);
        assert!((volume - 36.0).abs() < 1e-3, "got {volume} mm3");
    }

    #[test]
    fn the_outline_is_carried_round_a_bend_without_twisting() {
        let legs = square(vec![
            Vec3::ZERO,
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(10.0, 0.0, 8.0),
            Vec3::new(10.0, 6.0, 8.0),
        ])
        .legs();
        for (leg, pair) in legs.iter().zip(legs.windows(2)) {
            let axis = (leg.to - leg.from).normalize();
            assert!(
                leg.corner.dot(axis).abs() < 1e-5,
                "a corner is square to its leg"
            );
            assert_eq!(pair[0].end, pair[1].start, "two legs share their mitre");
        }
        assert!(
            legs[0].start.is_none() && legs[2].end.is_none(),
            "the ends are open"
        );
    }

    #[test]
    fn a_turn_sharper_than_a_right_angle_is_made_in_two() {
        let back = square(vec![
            Vec3::ZERO,
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(2.0, 3.0, 0.0),
        ]);
        let legs = back.legs();
        assert_eq!(legs.len(), 3, "the sharp turn is split by a short leg");
        for pair in legs.windows(2) {
            let (into, out) = (pair[0].to - pair[0].from, pair[1].to - pair[1].from);
            assert!(into.angle_between(out) <= SHARPEST_TURN + 1e-4);
        }
        let report = diagnose(&tunnel(&back).expect("it digs"));
        assert!(
            report.is_sound() && report.unbalanced_edges == 0,
            "{report:?}"
        );
    }
}
