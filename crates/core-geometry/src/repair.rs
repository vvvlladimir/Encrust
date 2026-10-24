//! The repairs that change a mesh only when they are asked to: closing the holes in it,
//! and dropping the faces laid over one another. See `docs/design/mesh-repair.md`.

use crate::hash::FastMap;
use crate::topology::{edge_groups, edge_uses};
use crate::triangulate::triangulate;
use crate::validate::duplicate_faces;
use rayon::prelude::*;

use crate::{Mesh, Scalar, Vec2, Vec3};

/// Smallest cross product treated as a direction, mm². Below it the loop is a line and
/// has no plane to be triangulated in.
const MIN_NORMAL: Scalar = 1e-12;

/// Longest loop whose patch is chosen by area. The choice costs the cube of the corner
/// count, so a longer loop, which in practice is a flat cut, is ear clipped in its plane.
const MAX_MINIMAL_RING: usize = 256;

/// Thinnest triangle a patch may hold, as twice its area over its longest edge squared:
/// below it the triangle is a sliver that would read as a face with no area.
const MIN_ASPECT: f64 = 1e-6;

/// How near two corners of a face with no area must stand to be one point, mm. Two cells'
/// vertices on one lattice point differ in the last bits of an `f32`, far under this; a
/// sliver whose corners are further apart is left whole.
const COINCIDENT_MM: Scalar = 1e-4;

/// Welds the corners of a zero-area face on one point and drops degenerate faces; returns the
/// count. Collapsing a zero-length edge keeps a closed surface closed.
pub fn collapse_flat_faces(mesh: &mut Mesh) -> usize {
    let mut onto: FastMap<u32, u32> = FastMap::default();
    for face in flat_faces(mesh) {
        for (one, other) in [(face[0], face[1]), (face[1], face[2]), (face[0], face[2])] {
            let gap = mesh.vertices[one as usize] - mesh.vertices[other as usize];
            if one != other && gap.length() <= COINCIDENT_MM {
                let (one, other) = (root(&onto, one), root(&onto, other));
                if one != other {
                    onto.insert(one.max(other), one.min(other));
                }
            }
        }
    }
    if onto.is_empty() {
        return 0;
    }
    for face in &mut mesh.faces {
        for corner in face.iter_mut() {
            *corner = root(&onto, *corner);
        }
    }
    let before = mesh.faces.len();
    mesh.faces
        .retain(|face| face[0] != face[1] && face[1] != face[2] && face[0] != face[2]);
    // A face the weld folded onto another is that one's reverse, and the two go together.
    remove_duplicate_faces(mesh);
    before - mesh.faces.len()
}

/// The corner `corner` was welded onto, through every weld after it.
fn root(onto: &FastMap<u32, u32>, mut corner: u32) -> u32 {
    while let Some(next) = onto.get(&corner) {
        corner = *next;
    }
    corner
}

/// The faces whose corners enclose no area at all.
fn flat_faces(mesh: &Mesh) -> Vec<[u32; 3]> {
    mesh.faces
        .par_iter()
        .filter(|face| {
            let [a, b, c] = face.map(|corner| mesh.vertices[corner as usize]);
            (b - a).cross(c - a).length_squared() == 0.0
        })
        .copied()
        .collect()
}

/// Drops the faces laid over others on the same three vertices without changing what the
/// surface encloses, and returns where they were in the mesh, ascending.
pub fn remove_duplicate_faces(mesh: &mut Mesh) -> Vec<u32> {
    let extra = duplicate_faces(mesh);
    if extra.is_empty() {
        return Vec::new();
    }
    let mut dropped = vec![false; mesh.faces.len()];
    for index in &extra {
        dropped[*index] = true;
    }
    let mut at = 0;
    mesh.faces.retain(|_| {
        let keep = !dropped[at];
        at += 1;
        keep
    });
    extra.into_iter().map(|index| index as u32).collect()
}

/// What closing the boundary loops of a mesh added to it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Filled {
    pub loops_filled: usize,
    pub faces_added: usize,
    /// Loops left open: a boundary that branches at a vertex has no one way round it.
    pub loops_left: usize,
}

impl Filled {
    /// Nothing was open, so the mesh is as it came.
    pub fn is_empty(&self) -> bool {
        self.loops_filled == 0 && self.loops_left == 0
    }
}

/// Closes every boundary loop of a mesh with a patch of triangles wound like the surface
/// around them.
///
/// Run [`crate::weld`] first: on an unwelded mesh every edge is a boundary. A loop with no
/// plane at all is left open and counted; see `docs/design/mesh-repair.md`.
pub fn fill_holes(mesh: &mut Mesh) -> Filled {
    let (rings, loops_left) = boundary_loops(mesh);
    let mut filled = Filled {
        loops_left,
        ..Filled::default()
    };
    for ring in rings {
        let added = patch(mesh, &ring);
        if added > 0 {
            filled.loops_filled += 1;
            filled.faces_added += added;
        } else {
            filled.loops_left += 1;
        }
    }
    filled
}

/// The mesh's boundary loops, wound as a closing patch is, and how many ends never came back round.
/// An odd-use edge cannot pair off without one more face; a fin is closed by its own reverse.
fn boundary_loops(mesh: &Mesh) -> (Vec<Vec<u32>>, usize) {
    let uses = edge_uses(mesh);
    let mut onwards: FastMap<u32, Vec<u32>> = FastMap::default();
    let mut starts = Vec::new();
    for group in edge_groups(&uses).filter(|group| group.len() % 2 == 1) {
        let forward = group.iter().filter(|edge_use| edge_use.forward).count();
        let backward = group.len() - forward;
        let (low, high) = group[0].edge;
        let (from, to) = if forward > backward {
            (high, low)
        } else {
            (low, high)
        };
        for _ in 0..forward.abs_diff(backward) {
            onwards.entry(from).or_default().push(to);
            starts.push(from);
        }
    }

    let mut rings = Vec::new();
    let mut left = 0;
    for start in starts {
        if onwards.get(&start).is_none_or(Vec::is_empty) {
            continue;
        }
        match walk(&mut onwards, start) {
            Some(ring) => rings.push(ring),
            None => left += 1,
        }
    }
    (rings, left)
}

/// Follows boundary edges from `start` until they come back to it, taking each edge only
/// once. `None` for a walk that runs out of edges first, which a branching boundary does.
fn walk(onwards: &mut FastMap<u32, Vec<u32>>, start: u32) -> Option<Vec<u32>> {
    let mut ring = vec![start];
    let mut at = start;
    loop {
        let to = onwards.get_mut(&at).and_then(Vec::pop)?;
        if to == start {
            return Some(ring);
        }
        ring.push(to);
        at = to;
    }
}

/// Lays triangles over one loop and returns how many it added, or zero for a loop with no
/// plane to lay them in.
fn patch(mesh: &mut Mesh, ring: &[u32]) -> usize {
    if ring.len() < 3 {
        return 0;
    }
    let points: Vec<Vec3> = ring
        .iter()
        .map(|index| *mesh.vertices.get(*index as usize).unwrap_or(&Vec3::ZERO))
        .collect();
    let minimal = (ring.len() <= MAX_MINIMAL_RING)
        .then(|| least_area(&points))
        .flatten();
    let Some(faces) = minimal.or_else(|| ear_clipped(&points)) else {
        // A ring whose projection folds over itself would be a hole again if left short;
        // a fan at least closes it. One with no plane has nothing a fan could lie in.
        return if newell(&points).length_squared() < MIN_NORMAL {
            0
        } else {
            fan(mesh, ring)
        };
    };
    mesh.faces.extend(
        faces
            .iter()
            .map(|face| face.map(|corner| ring[corner as usize])),
    );
    faces.len()
}

/// The least-area triangulation of a loop of corners with no sliver, or `None` if none exists.
fn least_area(points: &[Vec3]) -> Option<Vec<[u32; 3]>> {
    let n = points.len();
    let mut cost = vec![0.0f64; n * n];
    let mut split = vec![0u32; n * n];
    for span in 2..n {
        for first in 0..n - span {
            let last = first + span;
            let mut best = (f64::INFINITY, 0);
            for middle in first + 1..last {
                let inner = cost[first * n + middle] + cost[middle * n + last];
                if inner >= best.0 {
                    continue;
                }
                if let Some(area) = weight(points[first], points[middle], points[last])
                    && inner + area < best.0
                {
                    best = (inner + area, middle as u32);
                }
            }
            cost[first * n + last] = best.0;
            split[first * n + last] = best.1;
        }
    }
    if !cost[n - 1].is_finite() {
        return None;
    }

    let mut faces = Vec::with_capacity(n - 2);
    let mut spans = vec![(0, n - 1)];
    while let Some((first, last)) = spans.pop() {
        if last - first < 2 {
            continue;
        }
        let middle = split[first * n + last] as usize;
        faces.push([first as u32, middle as u32, last as u32]);
        spans.push((first, middle));
        spans.push((middle, last));
    }
    Some(faces)
}

/// The area of a triangle the patch may use, mm², or `None` for a sliver.
fn weight(a: Vec3, b: Vec3, c: Vec3) -> Option<f64> {
    let twice = f64::from((b - a).cross(c - a).length());
    let longest = (b - a)
        .length_squared()
        .max((c - b).length_squared())
        .max((a - c).length_squared());
    (twice > MIN_ASPECT * f64::from(longest)).then_some(twice / 2.0)
}

/// The loop ear clipped in its own plane, or `None` when it has none or its projection
/// folds over itself.
fn ear_clipped(points: &[Vec3]) -> Option<Vec<[u32; 3]>> {
    let normal = newell(points);
    if normal.length_squared() < MIN_NORMAL {
        return None;
    }
    let across = normal.normalize().any_orthonormal_vector();
    let up = normal.normalize().cross(across);
    let flat: Vec<Vec2> = points
        .iter()
        .map(|point| Vec2::new(point.dot(across), point.dot(up)))
        .collect();
    let corners: Vec<u32> = (0..points.len() as u32).collect();
    let faces = triangulate(&corners, &[], &flat);
    (faces.len() + 2 == points.len()).then_some(faces)
}

/// Newell's normal: the area-weighted normal of a loop that need not be flat, and
/// independent of where the loop stands.
fn newell(points: &[Vec3]) -> Vec3 {
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .map(|(point, next)| point.cross(*next))
        .sum()
}

/// Closes the loop with a fan from one new vertex in its middle, which is what a loop too
/// long, too folded or too tangled to triangulate otherwise gets.
fn fan(mesh: &mut Mesh, ring: &[u32]) -> usize {
    let middle: Vec3 = ring
        .iter()
        .filter_map(|index| mesh.vertices.get(*index as usize))
        .copied()
        .sum::<Vec3>()
        / ring.len() as Scalar;
    let hub = mesh.vertices.len() as u32;
    mesh.vertices.push(middle);
    for (at, corner) in ring.iter().enumerate() {
        mesh.faces.push([*corner, ring[(at + 1) % ring.len()], hub]);
    }
    ring.len()
}

#[cfg(test)]
mod tests;
