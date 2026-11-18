use super::{Face, shared};
use crate::{Scalar, Triangle, Vec2, Vec3};

/// An edge the walk crosses, laid flat, its ends named as the walk faces it.
pub(super) struct Portal {
    pub left: Vec2,
    pub right: Vec2,
    pub left_at: Vec3,
    pub right_at: Vec3,
}

/// A corridor laid flat in one plane, each face hinged onto the one before across their edge.
pub(super) struct Strip {
    pub start: Vec2,
    pub end: Vec2,
    pub portals: Vec<Portal>,
}

/// A corner of a face laid flat: its index, where it lies flat and where it stands placed.
#[derive(Clone, Copy)]
struct Corner {
    id: u32,
    flat: Vec2,
    placed: Vec3,
}

/// The corridor `faces` laid flat, with `from` on its first face and `to` on its last.
pub(super) fn unfold(faces: &[Face], from: Vec3, to: Vec3) -> Option<Strip> {
    let mut flat = lay_first(faces.first()?);
    let start = inward(&flat, flatten(&flat, from));
    let mut portals = Vec::with_capacity(faces.len().saturating_sub(1));
    for pair in faces.windows(2) {
        let link = shared(pair[0].ids, pair[1].ids)?;
        let corner = |id: u32| flat.iter().find(|corner| corner.id == id).copied();
        let (a, b) = (corner(link.edge[0])?, corner(link.edge[1])?);
        let behind = corner(link.apex_of_first)?.flat;
        let ahead_at = pair[1].corner(link.apex_of_second)?;
        let ahead = hinge(
            a.flat,
            b.flat,
            a.placed.distance(ahead_at),
            b.placed.distance(ahead_at),
            behind,
        );
        portals.push(portal(a, b, behind));
        let ahead = Corner {
            id: link.apex_of_second,
            flat: ahead,
            placed: ahead_at,
        };
        flat = [a, b, ahead];
    }
    Some(Strip {
        start,
        end: inward(&flat, flatten(&flat, to)),
        portals,
    })
}

fn lay_first(face: &Face) -> [Corner; 3] {
    let [a, b, c] = face.at;
    let b_flat = Vec2::new(a.distance(b), 0.0);
    let c_flat = hinge(
        Vec2::ZERO,
        b_flat,
        a.distance(c),
        b.distance(c),
        Vec2::NEG_Y,
    );
    let flat = [Vec2::ZERO, b_flat, c_flat];
    std::array::from_fn(|index| Corner {
        id: face.ids[index],
        flat: flat[index],
        placed: face.at[index],
    })
}

/// The point `to_a` from `a` and `to_b` from `b`, on the side of `ab` away from `behind`.
fn hinge(a: Vec2, b: Vec2, to_a: Scalar, to_b: Scalar, behind: Vec2) -> Vec2 {
    let edge = b - a;
    let length = edge.length();
    if length <= Scalar::EPSILON {
        return a;
    }
    let along = edge / length;
    let foot = (to_a * to_a - to_b * to_b + length * length) / (2.0 * length);
    let height = (to_a * to_a - foot * foot).max(0.0).sqrt();
    let side = if along.perp_dot(behind - a) > 0.0 {
        -1.0
    } else {
        1.0
    };
    a + along * foot + along.perp() * height * side
}

/// The edge from `a` to `b` as a walk coming from `behind` meets it.
fn portal(a: Corner, b: Corner, behind: Vec2) -> Portal {
    let facing = (a.flat + b.flat) * 0.5 - behind;
    let (left, right) = if facing.perp_dot(a.flat - behind) > 0.0 {
        (a, b)
    } else {
        (b, a)
    };
    Portal {
        left: left.flat,
        right: right.flat,
        left_at: left.placed,
        right_at: right.placed,
    }
}

/// How far toward its face's centre an end is moved, of the way there: an end on an edge
/// would leave the funnel no width at its first portal, and this moves it by microns.
const INWARD: Scalar = 1e-5;

fn inward(flat: &[Corner; 3], point: Vec2) -> Vec2 {
    point.lerp(centre(flat), INWARD)
}

fn centre(flat: &[Corner; 3]) -> Vec2 {
    (flat[0].flat + flat[1].flat + flat[2].flat) / 3.0
}

/// Where `point`, lying on the face laid flat as `flat`, falls on it flat: by its
/// barycentric weights, or the face's centre for a face with no area.
fn flatten(flat: &[Corner; 3], point: Vec3) -> Vec2 {
    let [a, b, c] = flat.map(|corner| corner.placed);
    match Triangle::new(a, b, c).barycentric(point) {
        Some(weights) => flat
            .iter()
            .zip(weights)
            .map(|(corner, weight)| corner.flat * weight)
            .sum(),
        None => centre(flat),
    }
}
