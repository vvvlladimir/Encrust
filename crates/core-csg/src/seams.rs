use std::collections::HashMap;
use std::hash::BuildHasherDefault;
use std::ops::Range;
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};

use core_geometry::glam::DVec3;
use core_geometry::{Components, FastMap, Mesh, point_key};
use rayon::prelude::*;

use crate::error::CsgError;

/// How far apart the copies of a split vertex are drawn, in millimetres: far under a pixel
/// of any panel, far over the engine's own tolerance on a model a printer holds.
const NUDGE_MM: f64 = 1e-4;

/// How many times the faces round a stuck seam are paired another way before giving up.
const ROUNDS: usize = 4;

/// A mesh as the engine reads it: positions three to a vertex and indices three to a face.
pub(crate) struct Unzipped {
    pub positions: Vec<f64>,
    pub indices: Vec<usize>,
}

/// `mesh` welded on exact positions, with every seam where shells touch along an edge or at
/// a vertex split so each shell has its own copy; see `docs/design/csg.md`.
pub(crate) fn unzipped(mesh: &Mesh) -> Result<Unzipped, CsgError> {
    let (points, faces) = welded(mesh);
    let uses = edge_uses(&faces);
    match whole(&points, &faces, &uses) {
        Some(unzipped) => Ok(unzipped),
        None => split(&points, &faces, &uses),
    }
}

/// The mesh with its seams split, paired another way round each stuck edge up to `ROUNDS`
/// times.
fn split(points: &[DVec3], faces: &[[usize; 3]], uses: &[EdgeUse]) -> Result<Unzipped, CsgError> {
    let mut seams = Seams::new(points, faces, uses)?;
    let mut stuck = Vec::new();
    for _ in 0..ROUNDS {
        let (origin, indices) = copies(faces, &seams.pairs);
        stuck = stuck_edges(&origin, &indices);
        if stuck.is_empty() {
            let positions = drawn_apart(points, faces, &origin, &indices);
            return Ok(Unzipped { positions, indices });
        }
        for edge in &stuck {
            seams.turn(*edge);
        }
    }
    Err(CsgError::NotSolid(format!(
        "{} edges where the surface branches could not be split into shells",
        stuck.len()
    )))
}

/// One corner's use of its edge: the edge's lower and higher vertex, the corner as
/// `3 * face + side`, and whether the face walks the edge from lower to higher.
///
/// `u32` halves what the sort moves; a mesh past four billion vertices is not printed.
type EdgeUse = (u32, u32, u32, bool);

/// Every corner's use of the edge it starts, sorted by edge, then face.
fn edge_uses(faces: &[[usize; 3]]) -> Vec<EdgeUse> {
    let mut uses: Vec<EdgeUse> = faces
        .par_iter()
        .enumerate()
        .flat_map_iter(|(face, corners)| {
            (0..3).map(move |side| {
                let (tail, head) = (corners[side], corners[(side + 1) % 3]);
                (
                    tail.min(head) as u32,
                    tail.max(head) as u32,
                    (3 * face + side) as u32,
                    tail < head,
                )
            })
        })
        .collect();
    uses.par_sort_unstable();
    uses
}

/// The mesh as `split` would answer it when there is nothing to split: every edge walked once
/// each way and every vertex carrying one fan. Vertices are numbered as `copies` numbers
/// them, in the order of the corners, and placed as `drawn_apart` places an unmoved copy.
fn whole(points: &[DVec3], faces: &[[usize; 3]], uses: &[EdgeUse]) -> Option<Unzipped> {
    let across: Vec<AtomicU32> = (0..uses.len()).map(|_| AtomicU32::new(u32::MAX)).collect();
    let paired = uses
        .par_chunk_by(|a, b| (a.0, a.1) == (b.0, b.1))
        .all(|run| match run {
            [a, b] if a.3 != b.3 => {
                across[a.2 as usize].store(b.2, Relaxed);
                across[b.2 as usize].store(a.2, Relaxed);
                true
            }
            _ => false,
        });
    if !paired || !one_fan_each(points.len(), faces, &across) {
        return None;
    }
    let mut number = vec![usize::MAX; points.len()];
    let mut origin = Vec::with_capacity(points.len());
    let indices = faces
        .iter()
        .flatten()
        .map(|&vertex| {
            if number[vertex] == usize::MAX {
                number[vertex] = origin.len();
                origin.push(vertex);
            }
            number[vertex]
        })
        .collect();
    // Adding the zero `drawn_apart` adds keeps a -0.0 the same 0.0 it would be there.
    let positions = origin
        .iter()
        .flat_map(|&vertex| (points[vertex] + DVec3::ZERO * NUDGE_MM).to_array())
        .collect();
    Some(Unzipped { positions, indices })
}

/// Whether the corners at each vertex form one fan, turning round it from face to face
/// across the edges `across` pairs.
fn one_fan_each(vertices: usize, faces: &[[usize; 3]], across: &[AtomicU32]) -> bool {
    let next = |corner: usize| corner - corner % 3 + (corner + 1) % 3;
    let vertex = |corner: usize| faces[corner / 3][corner % 3];
    let (count, first): (Vec<AtomicU32>, Vec<AtomicU32>) = (0..vertices)
        .map(|_| (AtomicU32::new(0), AtomicU32::new(u32::MAX)))
        .unzip();
    (0..across.len()).into_par_iter().for_each(|corner| {
        count[vertex(corner)].fetch_add(1, Relaxed);
        first[vertex(corner)].fetch_min(corner as u32, Relaxed);
    });
    (0..vertices).into_par_iter().all(|at| {
        let start = first[at].load(Relaxed);
        if start == u32::MAX {
            return true;
        }
        let (mut corner, mut fan) = (start as usize, 0u32);
        loop {
            corner = next(across[corner].load(Relaxed) as usize);
            fan += 1;
            if corner == start as usize || fan as usize > across.len() {
                break;
            }
        }
        fan == count[at].load(Relaxed)
    })
}

/// Vertices on the same exact point merged, and faces left without area by that dropped.
fn welded(mesh: &Mesh) -> (Vec<DVec3>, Vec<[usize; 3]>) {
    let mut index =
        FastMap::with_capacity_and_hasher(mesh.vertices.len(), BuildHasherDefault::default());
    let mut points = Vec::with_capacity(mesh.vertices.len());
    let mut remap = Vec::with_capacity(mesh.vertices.len());
    for vertex in &mesh.vertices {
        let next = points.len();
        let at = *index.entry(point_key(*vertex)).or_insert(next);
        if at == next {
            points.push(vertex.as_dvec3());
        }
        remap.push(at);
    }
    let faces = mesh
        .faces
        .iter()
        .map(|face| face.map(|vertex| remap[vertex as usize]))
        .filter(|[a, b, c]| a != b && b != c && c != a)
        .collect();
    (points, faces)
}

/// Every edge's faces two by two, each pair the two sides of one wedge of solid.
struct Seams {
    /// Two faces and the vertices of the edge they share, lower first.
    pairs: Vec<(usize, usize, [usize; 2])>,
    /// Where in `pairs` each edge that more than two faces use keeps its own.
    branching: HashMap<[usize; 2], Range<usize>>,
}

impl Seams {
    fn new(points: &[DVec3], faces: &[[usize; 3]], uses: &[EdgeUse]) -> Result<Self, CsgError> {
        let mut pairs = Vec::with_capacity(uses.len() / 2);
        let mut branching = HashMap::new();
        for run in uses.chunk_by(|a, b| (a.0, a.1) == (b.0, b.1)) {
            let edge = [run[0].0 as usize, run[0].1 as usize];
            let around: Vec<(usize, bool)> = run
                .iter()
                .map(|&(.., corner, up)| (corner as usize / 3, up))
                .collect();
            let wedges = match around.as_slice() {
                [(first, up), (second, other_up)] if up != other_up => vec![(*first, *second)],
                _ => wedges_around(points, faces, edge, around)?,
            };
            if wedges.len() > 1 {
                branching.insert(edge, pairs.len()..pairs.len() + wedges.len());
            }
            pairs.extend(wedges.into_iter().map(|(face, other)| (face, other, edge)));
        }
        Ok(Self { pairs, branching })
    }

    /// Pairs the faces round a branching `edge` the next way: each wedge's opening face
    /// with the closing face of the wedge after it.
    fn turn(&mut self, edge: [usize; 2]) {
        if let Some(range) = self.branching.get(&edge) {
            let wedges = &mut self.pairs[range.clone()];
            let first = wedges[0].1;
            for at in 1..wedges.len() {
                wedges[at - 1].1 = wedges[at].1;
            }
            if let Some(last) = wedges.last_mut() {
                last.1 = first;
            }
        }
    }
}

/// The faces round an edge more than two use, paired across each wedge of solid.
///
/// Turning round the edge from its lower vertex to its higher, a face walking it that way
/// has its solid behind it and one walking it back has it ahead. On a sound surface the two
/// alternate; where it folds through itself they nest, and are matched as brackets are.
fn wedges_around(
    points: &[DVec3],
    faces: &[[usize; 3]],
    [low, high]: [usize; 2],
    mut around: Vec<(usize, bool)>,
) -> Result<Vec<(usize, usize)>, CsgError> {
    let opened: isize = around
        .iter()
        .map(|&(_, forward)| if forward { -1 } else { 1 })
        .sum();
    if opened != 0 {
        return Err(CsgError::NotSolid(format!(
            "the edge between vertices {low} and {high} is walked {} times more one way \
             than the other",
            opened.unsigned_abs()
        )));
    }
    let axis = (points[high] - points[low]).normalize_or_zero();
    let (across, up) = axis.any_orthonormal_pair();
    let angle = |face: usize| {
        let third = faces[face].into_iter().find(|&at| at != low && at != high);
        let off = third.map_or(DVec3::ZERO, |at| points[at] - points[low]);
        off.dot(up).atan2(off.dot(across))
    };
    around.sort_by(|a, b| angle(a.0).total_cmp(&angle(b.0)));

    // Started just past the deepest point, no face closes a wedge before one opens it.
    let (mut depth, mut deepest, mut start) = (0isize, 0isize, 0);
    for (at, &(_, forward)) in around.iter().enumerate() {
        depth += if forward { -1 } else { 1 };
        if depth < deepest {
            (deepest, start) = (depth, at + 1);
        }
    }
    let count = around.len();
    around.rotate_left(start % count);
    let (mut open, mut wedges) = (Vec::new(), Vec::with_capacity(count / 2));
    for (face, forward) in around {
        if !forward {
            open.push(face);
        } else if let Some(opener) = open.pop() {
            wedges.push((opener, face));
        }
    }
    Ok(wedges)
}

/// One vertex for each fan of corners that paired faces join, as the vertex each copies,
/// and every corner's copy.
fn copies(faces: &[[usize; 3]], pairs: &[(usize, usize, [usize; 2])]) -> (Vec<usize>, Vec<usize>) {
    let corner_of = |face: usize, vertex: usize| {
        face * 3 + faces[face].iter().position(|&at| at == vertex).unwrap_or(0)
    };
    // The corners of faces that share a vertex across a paired edge, joined into the fans
    // each copy of the vertex is to carry.
    let mut corners = Components::new(faces.len() * 3);
    for &(face, other, edge) in pairs {
        for vertex in edge {
            corners.union(
                corner_of(face, vertex) as u32,
                corner_of(other, vertex) as u32,
            );
        }
    }
    let (labels, count) = corners.labels();
    let mut origin = vec![usize::MAX; count];
    for (corner, &label) in labels.iter().enumerate() {
        if origin[label as usize] == usize::MAX {
            origin[label as usize] = faces[corner / 3][corner % 3];
        }
    }
    (
        origin,
        labels.into_iter().map(|label| label as usize).collect(),
    )
}

/// The edges, by the vertices they were split from, still not used by exactly two faces
/// wound against each other.
fn stuck_edges(origin: &[usize], indices: &[usize]) -> Vec<[usize; 2]> {
    let mut uses: Vec<(usize, usize, bool)> = indices
        .par_chunks(3)
        .flat_map_iter(|face| {
            (0..3).map(move |side| {
                let (tail, head) = (face[side], face[(side + 1) % 3]);
                (tail.min(head), tail.max(head), tail < head)
            })
        })
        .collect();
    uses.par_sort_unstable();
    let mut stuck: Vec<[usize; 2]> = uses
        .chunk_by(|a, b| (a.0, a.1) == (b.0, b.1))
        .filter(|run| !matches!(run, [a, b] if a.2 != b.2))
        .map(|run| {
            let (a, b) = (origin[run[0].0], origin[run[0].1]);
            [a.min(b), a.max(b)]
        })
        .collect();
    stuck.sort_unstable();
    stuck.dedup();
    stuck
}

/// The copies' positions, each copy of a split vertex moved a nudge into its own fan so the
/// engine, which welds on exact positions, keeps them apart.
fn drawn_apart(
    points: &[DVec3],
    faces: &[[usize; 3]],
    origin: &[usize],
    indices: &[usize],
) -> Vec<f64> {
    let mut copies = vec![0u32; points.len()];
    for &vertex in origin {
        copies[vertex] += 1;
    }
    let mut drift = vec![DVec3::ZERO; origin.len()];
    for (corner, &copy) in indices.iter().enumerate() {
        if copies[origin[copy]] > 1 {
            let centre = faces[corner / 3].map(|at| points[at]).iter().sum::<DVec3>() / 3.0;
            drift[copy] += (centre - points[origin[copy]]).normalize_or_zero();
        }
    }
    origin
        .iter()
        .zip(&drift)
        .flat_map(|(&vertex, drift)| {
            (points[vertex] + drift.normalize_or_zero() * NUDGE_MM).to_array()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use core_geometry::{Vec3, ball, cuboid};

    use super::*;

    fn both_ways(mesh: &Mesh) -> (Option<Unzipped>, Unzipped) {
        let (points, faces) = welded(mesh);
        let uses = edge_uses(&faces);
        let split = split(&points, &faces, &uses).expect("the mesh is a solid");
        (whole(&points, &faces, &uses), split)
    }

    #[test]
    fn a_mesh_with_no_seam_reads_as_splitting_it_would() {
        let mut mesh = ball(Vec3::new(-0.0, 1.0, 2.0), 3.0, 24, 32);
        mesh.append(&cuboid(Vec3::splat(10.0), Vec3::splat(12.0)));
        let (whole, split) = both_ways(&mesh);
        let whole = whole.expect("a ball and a box apart have no seam");
        assert_eq!(whole.indices, split.indices);
        let bits = |at: &[f64]| at.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
        assert_eq!(bits(&whole.positions), bits(&split.positions));
    }

    #[test]
    fn cubes_sharing_a_corner_are_left_to_the_seams() {
        let mut mesh = cuboid(Vec3::ZERO, Vec3::ONE);
        mesh.append(&cuboid(Vec3::ONE, Vec3::splat(2.0)));
        assert!(both_ways(&mesh).0.is_none(), "the corner carries two fans");
    }

    #[test]
    fn cubes_sharing_an_edge_are_left_to_the_seams() {
        let mut mesh = cuboid(Vec3::ZERO, Vec3::ONE);
        mesh.append(&cuboid(Vec3::new(1.0, 1.0, 0.0), Vec3::new(2.0, 2.0, 1.0)));
        assert!(both_ways(&mesh).0.is_none(), "the edge is used four times");
    }
}
