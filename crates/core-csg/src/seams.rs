use std::collections::HashMap;
use std::ops::Range;

use core_geometry::Mesh;
use core_geometry::glam::DVec3;

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
    let mut seams = Seams::new(&points, &faces)?;
    let mut stuck = Vec::new();
    for _ in 0..ROUNDS {
        let (origin, indices) = copies(&faces, &seams.pairs);
        stuck = stuck_edges(&origin, &indices);
        if stuck.is_empty() {
            let positions = drawn_apart(&points, &faces, &origin, &indices);
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

/// Vertices on the same exact point merged, and faces left without area by that dropped.
fn welded(mesh: &Mesh) -> (Vec<DVec3>, Vec<[usize; 3]>) {
    let mut index = HashMap::with_capacity(mesh.vertices.len());
    let mut points = Vec::with_capacity(mesh.vertices.len());
    let mut remap = Vec::with_capacity(mesh.vertices.len());
    for vertex in &mesh.vertices {
        let next = points.len();
        let at = *index
            .entry(vertex.to_array().map(f32::to_bits))
            .or_insert(next);
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
    fn new(points: &[DVec3], faces: &[[usize; 3]]) -> Result<Self, CsgError> {
        let mut uses: Vec<(usize, usize, usize, bool)> = faces
            .iter()
            .enumerate()
            .flat_map(|(face, corners)| {
                (0..3).map(move |side| {
                    let (tail, head) = (corners[side], corners[(side + 1) % 3]);
                    (tail.min(head), tail.max(head), face, tail < head)
                })
            })
            .collect();
        uses.sort_unstable();

        let mut pairs = Vec::with_capacity(uses.len() / 2);
        let mut branching = HashMap::new();
        for run in uses.chunk_by(|a, b| (a.0, a.1) == (b.0, b.1)) {
            let edge = [run[0].0, run[0].1];
            let around: Vec<(usize, bool)> = run.iter().map(|&(.., face, up)| (face, up)).collect();
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
    let mut corners = Corners::new(faces.len() * 3);
    for &(face, other, edge) in pairs {
        for vertex in edge {
            corners.union(corner_of(face, vertex), corner_of(other, vertex));
        }
    }

    let mut copy_of = vec![usize::MAX; faces.len() * 3];
    let (mut origin, mut indices) = (Vec::new(), Vec::with_capacity(faces.len() * 3));
    for corner in 0..faces.len() * 3 {
        let root = corners.find(corner);
        if copy_of[root] == usize::MAX {
            copy_of[root] = origin.len();
            origin.push(faces[corner / 3][corner % 3]);
        }
        indices.push(copy_of[root]);
    }
    (origin, indices)
}

/// The edges, by the vertices they were split from, still not used by exactly two faces
/// wound against each other.
fn stuck_edges(origin: &[usize], indices: &[usize]) -> Vec<[usize; 2]> {
    let mut uses: Vec<(usize, usize, bool)> = indices
        .chunks(3)
        .flat_map(|face| {
            (0..3).map(move |side| {
                let (tail, head) = (face[side], face[(side + 1) % 3]);
                (tail.min(head), tail.max(head), tail < head)
            })
        })
        .collect();
    uses.sort_unstable();
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

/// The corners of faces that share a vertex across a paired edge, joined into the fans each
/// copy of the vertex is to carry.
struct Corners {
    parent: Vec<usize>,
}

impl Corners {
    fn new(count: usize) -> Self {
        Self {
            parent: (0..count).collect(),
        }
    }

    fn find(&mut self, mut corner: usize) -> usize {
        while self.parent[corner] != corner {
            self.parent[corner] = self.parent[self.parent[corner]];
            corner = self.parent[corner];
        }
        corner
    }

    fn union(&mut self, a: usize, b: usize) {
        let (a, b) = (self.find(a), self.find(b));
        self.parent[a.max(b)] = a.min(b);
    }
}
