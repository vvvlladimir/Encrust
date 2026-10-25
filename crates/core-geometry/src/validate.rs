use serde::{Deserialize, Serialize};

use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

use rayon::iter::{
    IndexedParallelIterator, IntoParallelIterator, IntoParallelRefIterator, ParallelIterator,
};
use rayon::slice::ParallelSlice;

use crate::topology::{EdgeUse, counted_out, edge_uses, shells, sort_runs};
use crate::{Mesh, Scalar, Vec3};

/// What is structurally wrong, or right, with a mesh.
///
/// Run [`crate::weld`] first. On an unwelded mesh every edge looks like a boundary and
/// these numbers mean nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeshDiagnostics {
    pub vertices: usize,
    pub faces: usize,
    /// Faces with a repeated vertex or zero area.
    pub degenerate_faces: usize,
    /// Faces laid over others on the same three vertices that would have to go: both of a
    /// pair wound opposite ways, the extras of one wound the same way.
    pub duplicate_faces: usize,
    pub unreferenced_vertices: usize,
    /// Edges used by exactly one face: the mesh is open there.
    pub boundary_edges: usize,
    /// Edges used by three or more faces: the surface branches there.
    pub non_manifold_edges: usize,
    /// Edges more faces walk one way than the other: the surface does not close there.
    pub unbalanced_edges: usize,
    /// Connected groups of faces.
    pub shells: usize,
    /// V - E + F over referenced vertices. A closed sphere-like shell gives 2.
    pub euler_characteristic: i64,
}

impl MeshDiagnostics {
    /// A surface that closes a volume: every edge walked as often one way as the other. Branching
    /// alone does not open a mesh, e.g. two sheets along a seam; see `docs/design/mesh-repair.md`.
    pub fn is_closed(&self) -> bool {
        self.unbalanced_edges == 0
    }

    /// Nothing that would make slicing produce garbage: no hole, no face without area, none
    /// drawn twice. An edge two faces walk the same way is not one: see ADR 0269.
    pub fn is_sound(&self) -> bool {
        self.boundary_edges == 0 && self.degenerate_faces == 0 && self.duplicate_faces == 0
    }

    /// Unbalanced edges with no hole behind them, where faces lie folded onto each other.
    pub fn folded_edges(&self) -> usize {
        self.unbalanced_edges.saturating_sub(self.boundary_edges)
    }
}

/// Inspects the topology of a mesh without modifying it.
pub fn diagnose(mesh: &Mesh) -> MeshDiagnostics {
    let uses = edge_uses(mesh);
    let degenerate = degenerate_faces(mesh);
    let edges = EdgeTally::of(&uses, &degenerate);

    let shells = if mesh.faces.is_empty() {
        0
    } else {
        shells(mesh.faces.len(), &uses).count()
    };
    let referenced = referenced_vertices(mesh);

    MeshDiagnostics {
        vertices: mesh.vertices.len(),
        faces: mesh.faces.len(),
        degenerate_faces: degenerate.par_iter().filter(|face| **face).count(),
        duplicate_faces: count_duplicates(mesh),
        unreferenced_vertices: mesh.vertices.len() - referenced,
        boundary_edges: edges.boundary,
        non_manifold_edges: edges.non_manifold,
        unbalanced_edges: edges.unbalanced,
        shells,
        euler_characteristic: count(referenced) - count(edges.all) + count(mesh.faces.len()),
    }
}

/// How many distinct edges a mesh has, and how many of them are of each wrong kind.
#[derive(Default)]
struct EdgeTally {
    all: usize,
    boundary: usize,
    non_manifold: usize,
    unbalanced: usize,
}

impl EdgeTally {
    fn of(uses: &[EdgeUse], degenerate: &[bool]) -> Self {
        uses.par_chunk_by(|a, b| a.edge == b.edge)
            .map(|group| Self {
                all: 1,
                boundary: usize::from(group.len() == 1),
                non_manifold: usize::from(group.len() > 2),
                unbalanced: usize::from(!balanced(group, degenerate)),
            })
            .reduce(Self::default, |a, b| Self {
                all: a.all + b.all,
                boundary: a.boundary + b.boundary,
                non_manifold: a.non_manifold + b.non_manifold,
                unbalanced: a.unbalanced + b.unbalanced,
            })
    }
}

/// Whether one edge's uses pair off, as many faces walking it one way as the other.
///
/// A face with no area has no side to be on, so it counts as whichever direction the edge
/// is short of: a plane cut leaves one along every edge it splits, wound as it falls.
pub(crate) fn balanced(group: &[EdgeUse], degenerate: &[bool]) -> bool {
    let (mut forward, mut backward, mut either) = (0usize, 0usize, 0usize);
    for edge_use in group {
        match (degenerate[edge_use.face as usize], edge_use.forward) {
            (true, _) => either += 1,
            (false, true) => forward += 1,
            (false, false) => backward += 1,
        }
    }
    let gap = forward.abs_diff(backward);
    gap <= either && (either - gap) % 2 == 0
}

/// Six times the volume enclosed by the mesh, positive when faces wind outwards.
///
/// Meaningful only on a closed mesh; on an open one the result depends on where the
/// hole is. Six times, because dividing once at the end keeps the sum exact for longer.
pub fn signed_volume_x6(mesh: &Mesh) -> Scalar {
    // Summed in f64: two million f32 terms drift by most of a percent.
    let sum: f64 = mesh
        .triangles()
        .map(|t| f64::from(t.a.dot(t.b.cross(t.c))))
        .sum();
    sum as Scalar
}

/// Volume enclosed by a closed mesh, cubic millimetres.
pub fn signed_volume(mesh: &Mesh) -> Scalar {
    signed_volume_x6(mesh) / 6.0
}

/// Centre of mass of the solid a closed mesh encloses, in the mesh's own space. A mesh with no
/// volume gives its bounding-box centre; `None` only for no vertices.
pub fn center_of_mass(mesh: &Mesh) -> Option<Vec3> {
    let aabb = mesh.aabb()?;
    let middle = (aabb.mins + aabb.maxs) / 2.0;

    // Moments cancel against each other far from the origin, which f32 does not survive
    // on a model of a million faces.
    let (moment, weight) = mesh.triangles().fold(
        (glam::DVec3::ZERO, 0.0_f64),
        |(moment, weight), triangle| {
            let six_volume = f64::from(triangle.a.dot(triangle.b.cross(triangle.c)));
            let centre = (triangle.a + triangle.b + triangle.c).as_dvec3() / 4.0;
            (moment + centre * six_volume, weight + six_volume)
        },
    );

    // An enclosed volume under a millionth of the bounding box is noise, not a solid.
    let box_volume = f64::from((aabb.maxs - aabb.mins).element_product());
    if weight.abs() / 6.0 <= box_volume * 1e-6 {
        return Some(middle);
    }
    let centre = moment / weight;
    Some(Vec3::new(
        centre.x as Scalar,
        centre.y as Scalar,
        centre.z as Scalar,
    ))
}

/// Widens a count for the Euler arithmetic, which is the only signed value here.
fn count(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn referenced_vertices(mesh: &Mesh) -> usize {
    let seen: Vec<AtomicBool> = (0..mesh.vertices.len())
        .into_par_iter()
        .map(|_| AtomicBool::new(false))
        .collect();
    mesh.faces.par_iter().flatten().for_each(|index| {
        if let Some(slot) = seen.get(*index as usize) {
            slot.store(true, Relaxed);
        }
    });
    seen.par_iter().filter(|s| s.load(Relaxed)).count()
}

/// Which faces have a repeated vertex or no area, one flag per face.
///
/// A sliver with a tiny but non-zero area is not one: choosing that threshold is a slicing
/// concern, see docs/design/slicing.md when step 2 lands.
pub(crate) fn degenerate_faces(mesh: &Mesh) -> Vec<bool> {
    mesh.faces
        .par_iter()
        .enumerate()
        .map(|(index, face)| {
            face[0] == face[1]
                || face[1] == face[2]
                || face[0] == face[2]
                || mesh.triangle(index).is_none_or(|t| t.is_degenerate())
        })
        .collect()
}

fn count_duplicates(mesh: &Mesh) -> usize {
    duplicate_faces(mesh).len()
}

/// The faces to drop where several cover the same three vertices, ascending. Opposite-wound pairs
/// go whole; of the rest the first stays, so no edge is unbalanced.
pub(crate) fn duplicate_faces(mesh: &Mesh) -> Vec<usize> {
    let keyed = faces_by_corners(mesh);
    let mut dropped = Vec::new();
    for group in keyed
        .chunk_by(|a, b| a.0 == b.0)
        .filter(|group| group.len() > 1)
    {
        let forward = group.iter().filter(|face| face.1).count();
        let backward = group.len() - forward;
        let keep = match forward.cmp(&backward) {
            std::cmp::Ordering::Greater => group.iter().find(|face| face.1),
            std::cmp::Ordering::Less => group.iter().find(|face| !face.1),
            std::cmp::Ordering::Equal => None,
        }
        .map(|face| face.2);
        dropped.extend(
            group
                .iter()
                .map(|face| face.2)
                .filter(|index| Some(*index) != keep),
        );
    }
    dropped.sort_unstable();
    dropped
}

/// Every face under its corners in ascending order, whether it winds that way, and its index;
/// sorted. Counted by lowest corner and sorted within runs, as `edge_uses` does, for linear cost.
fn faces_by_corners(mesh: &Mesh) -> Vec<([u32; 3], bool, usize)> {
    let keyed: Vec<([u32; 3], bool, usize)> = mesh
        .faces
        .par_iter()
        .enumerate()
        .map(|(index, face)| {
            let mut key = *face;
            key.sort_unstable();
            let ascending = [key, [key[1], key[2], key[0]], [key[2], key[0], key[1]]];
            (key, ascending.contains(face), index)
        })
        .collect();
    let buckets = keyed
        .par_iter()
        .map(|face| face.0[0] as usize + 1)
        .max()
        .unwrap_or(0);
    let (order, starts) = counted_out(keyed.len(), buckets, |face| keyed[face].0[0] as usize);
    let mut sorted: Vec<([u32; 3], bool, usize)> = order
        .par_iter()
        .map(|face| keyed[face.load(Relaxed) as usize])
        .collect();
    sort_runs(&mut sorted, &starts);
    sorted
}

#[cfg(test)]
mod tests;
