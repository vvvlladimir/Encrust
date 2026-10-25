use rayon::prelude::*;

use crate::hash::{FastMap, FastSet};
use crate::topology::{edge_groups, edge_uses};
use crate::{Mesh, Scalar, Triangle, Vec3};

/// How far a vertex may stand off an open edge and still be stitched into it, mm. An
/// exporter rounding to a micron leaves seams far inside it; see `docs/design/mesh-repair.md`.
pub const DEFAULT_SEAM_TOLERANCE: Scalar = 0.01;

/// Passes over the open edges. Each face has one edge split a pass and three edges, and no
/// vertex is ever made, so a fourth pass finds nothing.
const MAX_PASSES: usize = 4;

/// What stitching the seams of a mesh changed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stitched {
    /// Open edges cut where a vertex of the other side of a seam stands on them.
    pub edges_split: usize,
    /// Vertices laid into those edges, each one already in the mesh.
    pub vertices_inserted: usize,
}

/// Closes the seams where one side's vertex stands on the other side's open edge, by
/// splitting that edge's face at it; no vertex is moved or made.
pub fn stitch_seams(mesh: &mut Mesh, tolerance_mm: Scalar) -> Stitched {
    let mut stitched = Stitched::default();
    if tolerance_mm <= 0.0 {
        return stitched;
    }
    for _ in 0..MAX_PASSES {
        let splits = splits(mesh, tolerance_mm);
        if splits.is_empty() {
            break;
        }
        stitched.edges_split += splits.len();
        stitched.vertices_inserted += splits.iter().map(|split| split.inside.len()).sum::<usize>();
        apply(mesh, &splits);
    }
    stitched
}

/// One open edge of one face and the vertices standing on it, in order from `from` to `to`.
struct Split {
    face: u32,
    from: u32,
    to: u32,
    inside: Vec<u32>,
}

/// An edge of one face to split for each face with any, the first found.
fn splits(mesh: &Mesh, tolerance_mm: Scalar) -> Vec<Split> {
    let open = open_edges(mesh);
    if open.is_empty() {
        return Vec::new();
    }
    let grid = OpenCorners::of(mesh, &open, tolerance_mm);
    let found: Vec<Split> = open
        .par_iter()
        .filter_map(|&(face, from, to)| {
            let inside = grid.on_edge(mesh, face, from, to, tolerance_mm);
            let split = Split {
                face,
                from,
                to,
                inside,
            };
            // A vertex in line with the far corner would leave a face with no area.
            let fan = split.fan(mesh)?;
            fan.iter()
                .all(|&triangle| has_area(mesh, triangle))
                .then_some(split)
        })
        .collect();
    let mut taken = FastSet::default();
    found
        .into_iter()
        .filter(|split| taken.insert(split.face))
        .collect()
}

/// Every edge only one face uses, as `(face, from, to)` in that face's own direction.
fn open_edges(mesh: &Mesh) -> Vec<(u32, u32, u32)> {
    let uses = edge_uses(mesh);
    edge_groups(&uses)
        .filter(|group| group.len() == 1)
        .map(|group| {
            let (low, high) = group[0].edge;
            let (from, to) = if group[0].forward {
                (low, high)
            } else {
                (high, low)
            };
            (group[0].face, from, to)
        })
        .collect()
}

/// The corners of the open edges, hashed on a grid with cells no finer than two
/// tolerances, so a point within one of a cell's centre is in it or a neighbour.
struct OpenCorners {
    cell_mm: Scalar,
    cells: FastMap<[i32; 3], Vec<u32>>,
    /// The open edges each corner is an end of, as `(from, to)`.
    edges: FastMap<u32, Vec<(u32, u32)>>,
}

impl OpenCorners {
    fn of(mesh: &Mesh, open: &[(u32, u32, u32)], tolerance_mm: Scalar) -> Self {
        let length: Scalar = open
            .iter()
            .map(|&(_, from, to)| mesh.vertices[from as usize].distance(mesh.vertices[to as usize]))
            .sum();
        let mean = length / open.len() as Scalar;
        let cell_mm = mean.max(2.0 * tolerance_mm).max(Scalar::EPSILON);
        let mut edges: FastMap<u32, Vec<(u32, u32)>> = FastMap::default();
        for &(_, from, to) in open {
            edges.entry(from).or_default().push((from, to));
            edges.entry(to).or_default().push((from, to));
        }
        let mut grid = Self {
            cell_mm,
            cells: FastMap::default(),
            edges,
        };
        for &corner in grid.edges.keys() {
            let cell = cell_of(mesh.vertices[corner as usize], cell_mm);
            grid.cells.entry(cell).or_default().push(corner);
        }
        grid
    }

    /// The corners strictly inside the edge and within `tolerance_mm` of it, ordered along it.
    /// The edge is walked in steps of half a cell, whose neighbourhoods cover the tolerance.
    fn on_edge(
        &self,
        mesh: &Mesh,
        face: u32,
        from: u32,
        to: u32,
        tolerance_mm: Scalar,
    ) -> Vec<u32> {
        let (a, b) = (mesh.vertices[from as usize], mesh.vertices[to as usize]);
        let length = a.distance(b);
        if length == 0.0 {
            return Vec::new();
        }
        let steps = (length / (self.cell_mm / 2.0)).ceil() as usize;
        let mut seen = FastSet::default();
        let mut inside: Vec<(Scalar, u32)> = Vec::new();
        for step in 0..=steps {
            let centre = cell_of(a.lerp(b, step as Scalar / steps as Scalar), self.cell_mm);
            for cell in neighbours(centre) {
                for &corner in self.cells.get(&cell).into_iter().flatten() {
                    if mesh.faces[face as usize].contains(&corner) || !seen.insert(corner) {
                        continue;
                    }
                    let point = mesh.vertices[corner as usize];
                    let along = (point - a).dot(b - a) / (length * length);
                    let nearest = a.lerp(b, along);
                    if along > 0.0
                        && along < 1.0
                        && point.distance(nearest) <= tolerance_mm
                        && self.runs_back_along(mesh, corner, a, b, tolerance_mm)
                    {
                        inside.push((along, corner));
                    }
                }
            }
        }
        inside.sort_by(|one, other| one.0.total_cmp(&other.0));
        inside.into_iter().map(|(_, corner)| corner).collect()
    }

    /// Whether `corner` ends an open edge walked against `a` to `b` and lying in its line,
    /// which is what the other side of a seam is; an open edge only crossing near is not.
    fn runs_back_along(
        &self,
        mesh: &Mesh,
        corner: u32,
        a: Vec3,
        b: Vec3,
        tolerance_mm: Scalar,
    ) -> bool {
        let line = (b - a).normalize();
        let off_line = |point: Vec3| {
            let along = point - a;
            (along - line * along.dot(line)).length()
        };
        self.edges
            .get(&corner)
            .into_iter()
            .flatten()
            .any(|&(from, to)| {
                let (start, end) = (mesh.vertices[from as usize], mesh.vertices[to as usize]);
                let far = if from == corner { end } else { start };
                (end - start).dot(line) < 0.0 && off_line(far) <= tolerance_mm
            })
    }
}

fn cell_of(at: Vec3, cell_mm: Scalar) -> [i32; 3] {
    (at / cell_mm).floor().as_ivec3().to_array()
}

fn neighbours([x, y, z]: [i32; 3]) -> impl Iterator<Item = [i32; 3]> {
    (-1..=1).flat_map(move |dx| {
        (-1..=1).flat_map(move |dy| (-1..=1).map(move |dz| [x + dx, y + dy, z + dz]))
    })
}

impl Split {
    /// The faces the split face becomes, fanned from its corner across the split edge
    /// through the vertices laid in; `None` when there is nothing to lay in.
    fn fan(&self, mesh: &Mesh) -> Option<Vec<[u32; 3]>> {
        if self.inside.is_empty() {
            return None;
        }
        let face = mesh.faces[self.face as usize];
        let apex = face
            .into_iter()
            .find(|&corner| corner != self.from && corner != self.to)?;
        let chain: Vec<u32> = std::iter::once(self.from)
            .chain(self.inside.iter().copied())
            .chain(std::iter::once(self.to))
            .collect();
        Some(
            chain
                .windows(2)
                .map(|pair| [pair[0], pair[1], apex])
                .collect(),
        )
    }
}

fn has_area(mesh: &Mesh, face: [u32; 3]) -> bool {
    let [a, b, c] = face.map(|corner| mesh.vertices[corner as usize]);
    !Triangle::new(a, b, c).is_degenerate()
}

/// Puts each split face's fan in its place, the first face where the face was.
fn apply(mesh: &mut Mesh, splits: &[Split]) {
    for split in splits {
        let Some(fan) = split.fan(mesh) else {
            continue;
        };
        mesh.faces[split.face as usize] = fan[0];
        mesh.faces.extend_from_slice(&fan[1..]);
    }
}

#[cfg(test)]
mod tests;
