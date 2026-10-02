use std::collections::HashMap;
use std::hash::BuildHasherDefault;

use crate::hash::FastMap;
use crate::{Mesh, Scalar, Vec3};

/// Distance below which two vertices are treated as one, millimetres.
pub const DEFAULT_WELD_TOLERANCE: Scalar = 1e-5;

/// Smallest usable grid cell, so that a zero tolerance still hashes finitely.
const MIN_CELL: Scalar = 1e-7;

/// Cell side as a multiple of the tolerance.
///
/// Two and a half, so that the eight cells nearest a vertex are the only ones worth
/// looking in: a match in any of the nineteen left out would have to be at least 1.25
/// tolerances away, which is further than a match can be. A cell of exactly one tolerance
/// would need all twenty-seven.
const CELL_SCALE: Scalar = 2.5;

/// How much of an unwelded mesh survives welding, as a divisor of its vertex count.
///
/// An STL writes three vertices per triangle and a closed surface has about twice as many
/// triangles as vertices, so a sixth of them come out the other side. Overshooting the
/// map costs memory; undershooting it costs a rehash of millions of entries.
const SURVIVING_SHARE: usize = 6;

/// Outcome of merging coincident vertices.
#[derive(Debug, Clone, PartialEq)]
pub struct Welded {
    pub mesh: Mesh,
    /// How many vertices disappeared into another one.
    pub vertices_merged: usize,
    /// Indices in the original mesh of the faces that were dropped because they collapsed
    /// to a line or point, or referenced a vertex that does not exist. Ascending.
    pub dropped: Vec<u32>,
}

impl Welded {
    pub fn faces_removed(&self) -> usize {
        self.dropped.len()
    }
}

/// Merges vertices closer together than `tolerance` and drops the faces that collapse.
///
/// STL stores each triangle's three vertices independently and exporters round them
/// per face, so an unwelded mesh has no usable topology at all: every edge looks like a
/// boundary. Welding is what makes the checks in [`crate::diagnose`] mean anything.
pub fn weld(mesh: &Mesh, tolerance: Scalar) -> Welded {
    let cell = tolerance.max(MIN_CELL) * CELL_SCALE;
    let radius = tolerance.max(0.0);
    let radius_sq = radius * radius;

    let mut buckets: Buckets = HashMap::with_capacity_and_hasher(
        mesh.vertices.len() / SURVIVING_SHARE + 1,
        BuildHasherDefault::default(),
    );
    let mut vertices: Vec<Vec3> = Vec::with_capacity(mesh.vertices.len() / SURVIVING_SHARE + 1);
    let mut remap: Vec<u32> = Vec::with_capacity(mesh.vertices.len());

    for vertex in &mesh.vertices {
        let nearby = probe_cells(*vertex, cell);
        let existing = nearby
            .iter()
            .filter_map(|key| buckets.get(key))
            .flatten()
            .copied()
            .find(|&i| vertices[i as usize].distance_squared(*vertex) <= radius_sq);

        let index = existing.unwrap_or_else(|| {
            let fresh = vertices.len() as u32;
            vertices.push(*vertex);
            buckets.entry(nearby[0]).or_default().push(fresh);
            fresh
        });
        remap.push(index);
    }

    let mut faces = Vec::with_capacity(mesh.faces.len());
    let mut dropped = Vec::new();
    for (index, face) in mesh.faces.iter().enumerate() {
        let Some(mapped) = remap_face(*face, &remap) else {
            dropped.push(index as u32);
            continue;
        };
        faces.push(mapped);
    }

    let vertices_merged = mesh.vertices.len() - vertices.len();
    Welded {
        mesh: Mesh::new(vertices, faces),
        vertices_merged,
        dropped,
    }
}

fn remap_face(face: [u32; 3], remap: &[u32]) -> Option<[u32; 3]> {
    let a = *remap.get(face[0] as usize)?;
    let b = *remap.get(face[1] as usize)?;
    let c = *remap.get(face[2] as usize)?;
    if a == b || b == c || a == c {
        return None;
    }
    Some([a, b, c])
}

/// The eight cells a match for `v` could be in, its own cell first.
///
/// Only cell representatives are stored, so a match lies within one cell of the query on
/// every axis. Which side of the query that is is decided by which half of its own cell
/// it sits in; see `CELL_SCALE` for why the other nineteen cells cannot hold one.
fn probe_cells(v: Vec3, cell: Scalar) -> [[i64; 3]; 8] {
    let scaled = v / cell;
    let mut home = [0i64; 3];
    let mut step = [0i64; 3];
    for axis in 0..3 {
        let floor = scaled[axis].floor();
        home[axis] = floor as i64;
        step[axis] = if scaled[axis] - floor < 0.5 { -1 } else { 1 };
    }

    let mut cells = [[0i64; 3]; 8];
    for (corner, out) in cells.iter_mut().enumerate() {
        for axis in 0..3 {
            out[axis] = home[axis]
                + if corner >> axis & 1 == 1 {
                    step[axis]
                } else {
                    0
                };
        }
    }
    cells
}

type Buckets = FastMap<[i64; 3], Vec<u32>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coincident_vertices_collapse_into_one() {
        let mesh = Mesh::new(
            vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::new(1e-9, 0.0, 0.0)],
            vec![[0, 1, 2], [3, 1, 2]],
        );
        let welded = weld(&mesh, DEFAULT_WELD_TOLERANCE);

        assert_eq!(welded.mesh.vertices.len(), 3);
        assert_eq!(welded.vertices_merged, 1);
        assert_eq!(welded.mesh.faces, vec![[0, 1, 2], [0, 1, 2]]);
    }

    #[test]
    fn vertices_further_apart_than_the_tolerance_are_kept() {
        let mesh = Mesh::new(vec![Vec3::ZERO, Vec3::new(1e-3, 0.0, 0.0)], Vec::new());
        assert_eq!(weld(&mesh, DEFAULT_WELD_TOLERANCE).mesh.vertices.len(), 2);
    }

    #[test]
    fn a_pair_just_inside_the_tolerance_merges_wherever_the_grid_falls() {
        let tolerance = DEFAULT_WELD_TOLERANCE;
        // Split evenly over the three axes, so the pair is 0.9 tolerances apart.
        let step = tolerance * 0.9 / (3.0 as Scalar).sqrt();

        // Walk the pair across a whole cell, so it straddles every boundary the search
        // decides which side of on the way.
        for walk in 0..64 {
            let first = Vec3::splat(walk as Scalar * tolerance * CELL_SCALE / 64.0);
            let mesh = Mesh::new(vec![first, first + Vec3::splat(step)], Vec::new());
            assert_eq!(
                weld(&mesh, tolerance).mesh.vertices.len(),
                1,
                "a pair 0.9 tolerances apart at {first} has to merge"
            );
        }
    }

    #[test]
    fn negative_and_positive_zero_are_the_same_point() {
        let mesh = Mesh::new(vec![Vec3::new(-0.0, -0.0, -0.0), Vec3::ZERO], Vec::new());
        assert_eq!(weld(&mesh, 0.0).mesh.vertices.len(), 1);
    }

    #[test]
    fn collapsed_face_is_dropped() {
        let mesh = Mesh::new(
            vec![Vec3::ZERO, Vec3::new(1e-9, 0.0, 0.0), Vec3::Y],
            vec![[0, 1, 2]],
        );
        let welded = weld(&mesh, DEFAULT_WELD_TOLERANCE);
        assert!(welded.mesh.faces.is_empty());
        assert_eq!(welded.dropped, vec![0]);
    }

    #[test]
    fn face_referencing_a_missing_vertex_is_dropped() {
        let mesh = Mesh::new(vec![Vec3::ZERO, Vec3::X, Vec3::Y], vec![[0, 1, 9]]);
        assert_eq!(weld(&mesh, DEFAULT_WELD_TOLERANCE).faces_removed(), 1);
    }
}
