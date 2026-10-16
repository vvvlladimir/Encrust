//! Where a layer's piece is first born on a surface: at a low point. Between two layers a
//! new piece can only appear where the surface turns from going down to going up.

use core_geometry::{FastMap, Mesh, Scalar, Vec3};

/// How much higher than a vertex its lowest neighbour may stand for the vertex to still be
/// low, millimetres: zero but float noise, so that a flat underside counts.
const FLAT_MM: Scalar = 1e-4;

/// Where on `mesh` a piece of a layer can be born: every vertex lower than all it shares a
/// face with, and the middle of every flat underside narrower than `widest_mm`. The mesh is
/// taken as welded, which every import leaves it.
pub(super) fn births(mesh: &Mesh, widest_mm: Scalar) -> Vec<Vec3> {
    let lowest = lowest_beside(mesh);
    let flat = |index: u32| {
        let beside = lowest
            .get(index as usize)
            .copied()
            .unwrap_or(Scalar::INFINITY);
        beside.is_finite() && (height(mesh, index) - beside).abs() <= FLAT_MM
    };
    let mut births: Vec<Vec3> = mesh
        .vertices
        .iter()
        .zip(&lowest)
        .filter(|(vertex, beside)| vertex.z < **beside - FLAT_MM)
        .map(|(vertex, _)| *vertex)
        .collect();
    births.extend(undersides(mesh, flat, widest_mm));
    births
}

/// How low the lowest vertex sharing a face with each vertex of `mesh` stands.
fn lowest_beside(mesh: &Mesh) -> Vec<Scalar> {
    let mut lowest = vec![Scalar::INFINITY; mesh.vertices.len()];
    let z = |index: u32| height(mesh, index);
    for face in &mesh.faces {
        for corner in 0..3 {
            let beside = z(face[(corner + 1) % 3]).min(z(face[(corner + 2) % 3]));
            if let Some(slot) = lowest.get_mut(face[corner] as usize) {
                *slot = slot.min(beside);
            }
        }
    }
    lowest
}

/// The height of vertex `index`, millimetres, or infinity where `mesh` has no such vertex.
fn height(mesh: &Mesh, index: u32) -> Scalar {
    mesh.vertices
        .get(index as usize)
        .map_or(Scalar::INFINITY, |v| v.z)
}

/// The middle of each flat underside: the vertices `flat` names, joined where they share an
/// edge, kept where the whole is narrower than `widest_mm`.
fn undersides(mesh: &Mesh, flat: impl Fn(u32) -> bool, widest_mm: Scalar) -> Vec<Vec3> {
    let mut joined = Joined::new(mesh.vertices.len());
    for face in &mesh.faces {
        for corner in 0..3 {
            let (a, b) = (face[corner], face[(corner + 1) % 3]);
            if flat(a) && flat(b) {
                joined.join(a, b);
            }
        }
    }
    let mut spans: FastMap<u32, (Vec3, Vec3)> = FastMap::default();
    for (index, vertex) in mesh.vertices.iter().enumerate() {
        if flat(index as u32) {
            let span = spans
                .entry(joined.root(index as u32))
                .or_insert((*vertex, *vertex));
            *span = (span.0.min(*vertex), span.1.max(*vertex));
        }
    }
    spans
        .into_values()
        .filter(|(lo, hi)| (hi.x - lo.x).max(hi.y - lo.y) < widest_mm)
        .map(|(lo, hi)| (lo + hi) / 2.0)
        .collect()
}

/// Vertices joined into groups, each named by its root.
struct Joined {
    parent: Vec<u32>,
}

impl Joined {
    fn new(count: usize) -> Self {
        Self {
            parent: (0..count as u32).collect(),
        }
    }

    fn root(&mut self, mut at: u32) -> u32 {
        while self.parent[at as usize] != at {
            let up = self.parent[self.parent[at as usize] as usize];
            self.parent[at as usize] = up;
            at = up;
        }
        at
    }

    fn join(&mut self, a: u32, b: u32) {
        let (a, b) = (self.root(a), self.root(b));
        if a != b {
            self.parent[a as usize] = b;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A square pyramid with its apex at the origin and its base `size_mm` across, 20 mm up.
    fn pyramid(size_mm: Scalar) -> Mesh {
        let half = size_mm / 2.0;
        Mesh::new(
            vec![
                Vec3::ZERO,
                Vec3::new(-half, -half, 20.0),
                Vec3::new(half, -half, 20.0),
                Vec3::new(half, half, 20.0),
                Vec3::new(-half, half, 20.0),
            ],
            vec![
                [0, 2, 1],
                [0, 3, 2],
                [0, 4, 3],
                [0, 1, 4],
                [1, 2, 3],
                [1, 3, 4],
            ],
        )
    }

    /// A slab `size_mm` square and 1 mm thick, its flat underside at height 5 mm.
    fn slab(size_mm: Scalar) -> Mesh {
        let mut vertices = Vec::new();
        for z in [5.0, 6.0] {
            for (x, y) in [
                (0.0, 0.0),
                (size_mm, 0.0),
                (size_mm, size_mm),
                (0.0, size_mm),
            ] {
                vertices.push(Vec3::new(x, y, z));
            }
        }
        let faces = vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        Mesh::new(vertices, faces)
    }

    #[test]
    fn a_piece_is_born_at_the_apex_of_a_pyramid_point_down() {
        assert_eq!(births(&pyramid(10.0), 1.5), vec![Vec3::ZERO]);
    }

    #[test]
    fn a_narrow_flat_underside_is_born_in_its_middle() {
        assert_eq!(
            births(&slab(1.0), 1.5),
            vec![Vec3::new(0.5, 0.5, 5.0)],
            "the middle of a 1 mm square underside"
        );
    }

    #[test]
    fn an_underside_wider_than_a_window_opens_none() {
        assert!(births(&slab(10.0), 1.5).is_empty());
    }
}
