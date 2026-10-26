use rayon::prelude::*;

use glam::{DVec2, DVec3};

use crate::{Aabb, Bvh, Mesh};

/// The pairs of faces of `mesh` that cross each other, lower face first, ascending.
///
/// Faces sharing a corner are never a pair, and faces only touching are not counted: the
/// test is strict, in `f64`; see `docs/design/mesh-repair.md`.
pub fn self_intersections(mesh: &Mesh, bvh: &Bvh) -> Vec<[u32; 2]> {
    let mut pairs: Vec<[u32; 2]> = (0..mesh.faces.len())
        .into_par_iter()
        .map_init(Vec::new, |near, face| {
            crossing(mesh, bvh, face as u32, near)
        })
        .flatten_iter()
        .collect();
    pairs.par_sort_unstable();
    pairs
}

/// The faces after `face` that cross it.
fn crossing(mesh: &Mesh, bvh: &Bvh, face: u32, near: &mut Vec<u32>) -> Vec<[u32; 2]> {
    let corners = mesh.faces[face as usize];
    let one = points(mesh, corners);
    let bounds = Aabb::new(
        one[0].min(one[1]).min(one[2]).as_vec3(),
        one[0].max(one[1]).max(one[2]).as_vec3(),
    );
    bvh.faces_meeting(&bounds, near);
    near.iter()
        .copied()
        .filter(|&other| other > face)
        .filter(|&other| {
            let others = mesh.faces[other as usize];
            !others.iter().any(|corner| corners.contains(corner))
                && cross(&one, &points(mesh, others))
        })
        .map(|other| [face, other])
        .collect()
}

fn points(mesh: &Mesh, face: [u32; 3]) -> Corners {
    face.map(|corner| mesh.vertices[corner as usize].as_dvec3())
}

type Corners = [DVec3; 3];

/// Whether two triangles cross: an edge of one passes through the other, or, lying in one
/// plane, their insides overlap.
fn cross(one: &Corners, other: &Corners) -> bool {
    let sides = other.map(|point| orient(one, point));
    if sides.iter().all(|&side| side == 0.0) {
        return overlap_flat(one, other);
    }
    edges(one).any(|edge| through(other, edge)) || edges(other).any(|edge| through(one, edge))
}

fn edges(triangle: &Corners) -> impl Iterator<Item = [DVec3; 2]> + '_ {
    (0..3).map(move |at| [triangle[at], triangle[(at + 1) % 3]])
}

/// Six times the signed volume of the triangle and `point`: which side of its plane it is.
fn orient(triangle: &Corners, point: DVec3) -> f64 {
    let [a, b, c] = *triangle;
    (b - a).cross(c - a).dot(point - a)
}

/// Whether the segment passes strictly through the triangle's inside.
fn through(triangle: &Corners, [from, to]: [DVec3; 2]) -> bool {
    if orient(triangle, from) * orient(triangle, to) >= 0.0 {
        return false;
    }
    let side = |a: DVec3, b: DVec3| (a - from).cross(b - from).dot(to - from);
    let [a, b, c] = *triangle;
    let turns = [side(a, b), side(b, c), side(c, a)];
    turns.iter().all(|&turn| turn > 0.0) || turns.iter().all(|&turn| turn < 0.0)
}

/// Whether two triangles in one plane overlap inside: two edges cross, or a corner of one
/// stands inside the other.
fn overlap_flat(one: &Corners, other: &Corners) -> bool {
    let normal = (one[1] - one[0]).cross(one[2] - one[0]).abs();
    let [u, v] = if normal.x >= normal.y && normal.x >= normal.z {
        [1, 2]
    } else if normal.y >= normal.z {
        [0, 2]
    } else {
        [0, 1]
    };
    let flat = |triangle: &Corners| triangle.map(|point| DVec2::new(point[u], point[v]));
    let (one, other) = (flat(one), flat(other));
    let edges_cross = (0..3).any(|at| {
        (0..3).any(|to| {
            segments_cross(
                [one[at], one[(at + 1) % 3]],
                [other[to], other[(to + 1) % 3]],
            )
        })
    });
    edges_cross
        || one.iter().any(|&point| inside(&other, point))
        || other.iter().any(|&point| inside(&one, point))
}

fn turn(a: DVec2, b: DVec2, point: DVec2) -> f64 {
    (b - a).perp_dot(point - a)
}

fn segments_cross([a, b]: [DVec2; 2], [c, d]: [DVec2; 2]) -> bool {
    turn(a, b, c) * turn(a, b, d) < 0.0 && turn(c, d, a) * turn(c, d, b) < 0.0
}

fn inside(triangle: &[DVec2; 3], point: DVec2) -> bool {
    let turns = [0, 1, 2].map(|at| turn(triangle[at], triangle[(at + 1) % 3], point));
    turns.iter().all(|&turn| turn > 0.0) || turns.iter().all(|&turn| turn < 0.0)
}

#[cfg(test)]
mod tests;
