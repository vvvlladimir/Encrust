use rayon::prelude::*;

use glam::{DVec2, DVec3, Mat4};

use crate::{Aabb, Bvh, Mesh, Transform, Vec3};

/// A mesh as it stands on the plate: its faces with their hierarchy in its own space, its
/// placement, and the box it stands in, plate millimetres.
#[derive(Debug, Clone, Copy)]
pub struct PlacedMesh<'a> {
    pub mesh: &'a Mesh,
    pub bvh: &'a Bvh,
    pub transform: Transform,
    pub bounds_mm: Aabb,
}

/// The pairs of faces of `mesh` that cross each other, lower face first, ascending; faces
/// sharing a corner or only touching are not. See `docs/design/mesh-repair.md`.
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

/// Whether a face of `one` crosses a face of `other`, each where it stands. Only faces in
/// the box both stand in are asked; a placement scaled flat crosses nothing.
pub fn placed_faces_cross(one: &PlacedMesh<'_>, other: &PlacedMesh<'_>) -> bool {
    let low = one.bounds_mm.mins.max(other.bounds_mm.mins);
    let high = one.bounds_mm.maxs.min(other.bounds_mm.maxs);
    if low.cmpgt(high).any() {
        return false;
    }
    let (Some(into_one), Some(into_other)) = (
        one.transform.inverse_matrix(),
        other.transform.inverse_matrix(),
    ) else {
        return false;
    };
    let mut near = Vec::new();
    one.bvh
        .faces_meeting(&box_through(into_one, low, high), &mut near);
    let one_to_other = into_other * one.transform.to_matrix();
    near.par_iter()
        .map_init(Vec::new, |found, &face| {
            let corners = one.mesh.faces[face as usize].map(|corner| {
                let vertex = one.mesh.vertices[corner as usize];
                one_to_other.transform_point3(vertex).as_dvec3()
            });
            other.bvh.faces_meeting(&bounds_of(&corners), found);
            found.iter().any(|&face| {
                cross(
                    &corners,
                    &points(other.mesh, other.mesh.faces[face as usize]),
                    Rim::Closed,
                )
            })
        })
        .any(|crossed| crossed)
}

/// The box the box `low`..`high` stands in once carried by `matrix`.
fn box_through(matrix: Mat4, low: Vec3, high: Vec3) -> Aabb {
    let corners = (0..8).map(|at| {
        let pick = |bit: usize, axis: usize| if at & bit == 0 { low[axis] } else { high[axis] };
        matrix.transform_point3(Vec3::new(pick(1, 0), pick(2, 1), pick(4, 2)))
    });
    let (low, high) = corners.fold(
        (Vec3::INFINITY, Vec3::NEG_INFINITY),
        |(low, high), corner| (low.min(corner), high.max(corner)),
    );
    Aabb::new(low, high)
}

fn bounds_of(triangle: &Corners) -> Aabb {
    Aabb::new(
        triangle[0].min(triangle[1]).min(triangle[2]).as_vec3(),
        triangle[0].max(triangle[1]).max(triangle[2]).as_vec3(),
    )
}

/// The faces after `face` that cross it.
fn crossing(mesh: &Mesh, bvh: &Bvh, face: u32, near: &mut Vec<u32>) -> Vec<[u32; 2]> {
    let corners = mesh.faces[face as usize];
    let one = points(mesh, corners);
    bvh.faces_meeting(&bounds_of(&one), near);
    near.iter()
        .copied()
        .filter(|&other| other > face)
        .filter(|&other| {
            let others = mesh.faces[other as usize];
            !others.iter().any(|corner| corners.contains(corner))
                && cross(&one, &points(mesh, others), Rim::Open)
        })
        .map(|other| [face, other])
        .collect()
}

fn points(mesh: &Mesh, face: [u32; 3]) -> Corners {
    face.map(|corner| mesh.vertices[corner as usize].as_dvec3())
}

type Corners = [DVec3; 3];

/// Whether an edge passing along a triangle's rim goes through it: not for faces of one mesh,
/// which meet there by design; so for two meshes, whose crossings at round offsets land there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rim {
    Open,
    Closed,
}

/// Whether two triangles cross: an edge of one passes through the other, or, lying in one
/// plane, their insides overlap.
fn cross(one: &Corners, other: &Corners, rim: Rim) -> bool {
    let sides = other.map(|point| orient(one, point));
    if sides.iter().all(|&side| side == 0.0) {
        return overlap_flat(one, other);
    }
    edges(one).any(|edge| through(other, edge, rim))
        || edges(other).any(|edge| through(one, edge, rim))
}

fn edges(triangle: &Corners) -> impl Iterator<Item = [DVec3; 2]> + '_ {
    (0..3).map(move |at| [triangle[at], triangle[(at + 1) % 3]])
}

/// Six times the signed volume of the triangle and `point`: which side of its plane it is.
fn orient(triangle: &Corners, point: DVec3) -> f64 {
    let [a, b, c] = *triangle;
    (b - a).cross(c - a).dot(point - a)
}

/// Whether the segment, its ends strictly either side, passes through the triangle's inside,
/// or with `Rim::Closed` its rim.
fn through(triangle: &Corners, [from, to]: [DVec3; 2], rim: Rim) -> bool {
    if orient(triangle, from) * orient(triangle, to) >= 0.0 {
        return false;
    }
    let side = |a: DVec3, b: DVec3| (a - from).cross(b - from).dot(to - from);
    let [a, b, c] = *triangle;
    let turns = [side(a, b), side(b, c), side(c, a)];
    match rim {
        Rim::Open => turns.iter().all(|&turn| turn > 0.0) || turns.iter().all(|&turn| turn < 0.0),
        Rim::Closed => {
            turns.iter().all(|&turn| turn >= 0.0) || turns.iter().all(|&turn| turn <= 0.0)
        }
    }
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
