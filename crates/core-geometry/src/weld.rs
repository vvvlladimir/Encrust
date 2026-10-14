use rayon::iter::{
    IndexedParallelIterator, IntoParallelIterator, IntoParallelRefIterator, ParallelIterator,
};
use rayon::slice::ParallelSliceMut;

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
    let first = first_copies(&mesh.vertices);
    let (vertices, remap) = merge_nearby(&mesh.vertices, &first, tolerance);

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

/// For every vertex, the lowest index holding the same bits.
///
/// Exporters write a shared corner with the same bits each time, so this finds most merges
/// by a parallel sort; `-0.0` and `0.0` are left to the grid.
fn first_copies(vertices: &[Vec3]) -> Vec<u32> {
    // Sorted with the bits beside the index rather than looked up through it, which on a
    // million vertices is the difference between a sort and a walk of cache misses.
    let mut keyed: Vec<([u32; 3], u32)> = vertices
        .iter()
        .enumerate()
        .map(|(at, vertex)| (vertex.to_array().map(Scalar::to_bits), at as u32))
        .collect();
    keyed.par_sort_unstable();

    let mut first = vec![0u32; vertices.len()];
    for run in keyed.chunk_by(|a, b| a.0 == b.0) {
        for (_, at) in run {
            first[*at as usize] = run[0].1;
        }
    }
    first
}

/// Merges points closer together than `tolerance` into the first of them a grid finds,
/// and returns what is left and where each point went.
///
/// A distinct point with no other in the eight cells a match could be in merges with
/// nothing; those are told apart in parallel, and only the rest go through the grid.
fn merge_nearby(points: &[Vec3], first: &[u32], tolerance: Scalar) -> (Vec<Vec3>, Vec<u32>) {
    let cell = tolerance.max(MIN_CELL) * CELL_SCALE;
    let is_first = |at: usize| first[at] as usize == at;
    let mut occupied: Vec<[i64; 3]> = points
        .par_iter()
        .enumerate()
        .filter(|(at, _)| is_first(*at))
        .map(|(_, point)| probe_cells(*point, cell)[0])
        .collect();
    occupied.par_sort_unstable();
    let company: Vec<bool> = points
        .par_iter()
        .enumerate()
        .map(|(at, point)| is_first(at) && has_company(&occupied, &probe_cells(*point, cell)))
        .collect();
    // A copy of a crowded point is searched again, as it was before copies were split off.
    let crowded: Vec<u32> = (0..points.len() as u32)
        .into_par_iter()
        .filter(|at| company[first[*at as usize] as usize])
        .collect();

    let mut rep = first.to_vec();
    for (at, onto) in crowded
        .iter()
        .zip(merge_crowded(points, &crowded, tolerance))
    {
        rep[*at as usize] = onto;
    }
    // Every point goes to one before it or to itself, so one pass in order numbers them.
    let mut kept = Vec::new();
    for at in 0..points.len() {
        let onto = rep[at] as usize;
        rep[at] = if onto == at {
            kept.push(points[at]);
            kept.len() as u32 - 1
        } else {
            rep[onto]
        };
    }
    (kept, rep)
}

/// Whether a point shares its own cell, or any of the other seven a match could be in,
/// with another point.
fn has_company(occupied: &[[i64; 3]], probe: &[[i64; 3]; 8]) -> bool {
    let home = occupied.partition_point(|cell| cell < &probe[0]);
    occupied.get(home + 1) == Some(&probe[0])
        || probe[1..]
            .iter()
            .any(|cell| occupied.binary_search(cell).is_ok())
}

/// Puts the crowded points through the grid in their order, and returns the point each
/// merged into, itself for one that is kept.
fn merge_crowded(points: &[Vec3], crowded: &[u32], tolerance: Scalar) -> Vec<u32> {
    let cell = tolerance.max(MIN_CELL) * CELL_SCALE;
    let radius_sq = tolerance.max(0.0) * tolerance.max(0.0);
    let mut cells = Cells::default();
    let mut onto = Vec::with_capacity(crowded.len());

    for &at in crowded {
        let point = points[at as usize];
        let nearby = probe_cells(point, cell);
        let existing = nearby
            .iter()
            .filter_map(|key| cells.get(key))
            .flatten()
            .copied()
            .find(|&other| points[other as usize].distance_squared(point) <= radius_sq);

        onto.push(existing.unwrap_or_else(|| {
            cells.entry(nearby[0]).or_default().push(at);
            at
        }));
    }
    onto
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

type Cells = FastMap<[i64; 3], Vec<u32>>;

#[cfg(test)]
mod tests {
    use super::*;

    /// Every point through the grid in turn, as welding did before it split the lonely
    /// points off: what the split has to reproduce exactly.
    fn one_by_one(points: &[Vec3], tolerance: Scalar) -> (Vec<Vec3>, Vec<u32>) {
        let cell = tolerance.max(MIN_CELL) * CELL_SCALE;
        let radius_sq = tolerance * tolerance;
        let mut cells: FastMap<[i64; 3], Vec<u32>> = FastMap::default();
        let (mut kept, mut onto) = (Vec::<Vec3>::new(), Vec::new());
        for point in points {
            let nearby = probe_cells(*point, cell);
            let found = nearby
                .iter()
                .filter_map(|key| cells.get(key))
                .flatten()
                .copied()
                .find(|&i| kept[i as usize].distance_squared(*point) <= radius_sq);
            onto.push(found.unwrap_or_else(|| {
                kept.push(*point);
                cells
                    .entry(nearby[0])
                    .or_default()
                    .push(kept.len() as u32 - 1);
                kept.len() as u32 - 1
            }));
        }
        (kept, onto)
    }

    #[test]
    fn welding_in_parallel_merges_exactly_what_one_point_at_a_time_did() {
        let tolerance = DEFAULT_WELD_TOLERANCE;
        // Mostly lone points, with clusters strung out in steps of 0.7 tolerances, so a
        // chain is longer than any one merge may reach.
        let mut points = Vec::new();
        for at in 0..20_000u32 {
            let spot = Vec3::new(
                (at % 97) as Scalar,
                (at / 97 % 89) as Scalar,
                (at % 7) as Scalar,
            );
            points.push(spot * 0.01);
            if at % 11 == 0 {
                for step in 1..5 {
                    points.push(spot * 0.01 + Vec3::X * tolerance * 0.7 * step as Scalar);
                }
            }
            // A copy of a point some way back, as an exporter writes a shared corner.
            if at % 13 == 0 && points.len() > 40 {
                points.push(points[points.len() - 37]);
            }
        }

        assert_eq!(
            merge_nearby(&points, &first_copies(&points), tolerance),
            one_by_one(&points, tolerance)
        );
    }

    #[test]
    fn a_point_near_two_kept_ones_in_a_cell_merges_into_the_older() {
        let tolerance = DEFAULT_WELD_TOLERANCE;
        let at = |share: Scalar| Vec3::new(share * tolerance, 0.5 * tolerance, 0.5 * tolerance);
        // Both kept points 1.5 tolerances apart in the cell from 0 to 2.5, and the third
        // within one tolerance of each; its copy comes after a closer point was kept.
        let points = vec![at(0.1), at(1.6), at(0.85), at(0.85)];

        let (kept, onto) = merge_nearby(&points, &first_copies(&points), tolerance);
        assert_eq!(kept.len(), 2);
        assert_eq!(onto, vec![0, 1, 0, 0], "the oldest point in the cell wins");
        assert_eq!((kept, onto), one_by_one(&points, tolerance));
    }

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
