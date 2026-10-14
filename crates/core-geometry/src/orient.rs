use std::cmp::Reverse;

use serde::{Deserialize, Serialize};

use crate::topology::{Components, agree, edge_groups, edge_uses};
use crate::{Mesh, Scalar};

/// What [`orient_outward`] had to change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Orientation {
    /// Faces whose winding differs from the input.
    pub flipped_faces: usize,
    /// Closed shells that were wound inwards and got turned inside out as a whole.
    pub inverted_shells: usize,
    /// False when some edge is still walked the same way by both its faces: a Mobius
    /// strip, or two bodies whose surfaces were written folded onto each other.
    pub orientable: bool,
}

impl Orientation {
    pub fn unchanged(&self) -> bool {
        self.flipped_faces == 0
    }
}

/// Makes every face wind the same way round, and turns closed shells outwards.
///
/// STL face normals are ignored throughout: exporters write them inconsistently, so
/// winding order is the only trustworthy source of orientation. Run [`crate::weld`]
/// first, or no two faces will look adjacent. Never leaves more edges walked the same way
/// by both their faces than it found; see `docs/design/mesh-repair.md`.
pub fn orient_outward(mesh: &mut Mesh) -> Orientation {
    if mesh.faces.is_empty() {
        return Orientation {
            flipped_faces: 0,
            inverted_shells: 0,
            orientable: true,
        };
    }

    let uses = edge_uses(mesh);
    let mut links = Vec::new();
    let mut touches_open_edge = vec![false; mesh.faces.len()];
    for group in edge_groups(&uses) {
        if let [first, second] = group {
            links.push(Link {
                faces: [first.face, second.face],
                agreed: agree(first, second),
            });
        } else {
            for edge_use in group {
                touches_open_edge[edge_use.face as usize] = true;
            }
        }
    }

    let mut flip = settle(mesh.faces.len(), &links);
    for (index, face) in mesh.faces.iter_mut().enumerate() {
        if flip[index] {
            face.swap(1, 2);
        }
    }

    let mut shells = Components::new(mesh.faces.len());
    for link in &links {
        shells.union(link.faces[0], link.faces[1]);
    }
    let (shell, shells) = shells.labels();
    let invert = shells_to_invert(mesh, &shell, shells, &touches_open_edge);
    let inverted_shells = invert.iter().filter(|wanted| **wanted).count();
    for (index, face) in mesh.faces.iter_mut().enumerate() {
        if invert[shell[index] as usize] {
            face.swap(1, 2);
            flip[index] = !flip[index];
        }
    }

    Orientation {
        flipped_faces: flip.iter().filter(|flipped| **flipped).count(),
        inverted_shells,
        orientable: links.iter().all(|link| {
            let [a, b] = link.faces.map(|face| flip[face as usize]);
            link.agreed == (a == b)
        }),
    }
}
/// Two faces sharing an edge no third face uses, and whether they walk it in opposite
/// directions as they stand.
struct Link {
    faces: [u32; 2],
    agreed: bool,
}

/// Which faces to turn round so that as few links as possible are walked the same way.
///
/// Faces that already agree with each other form patches, and a patch is turned as a
/// whole, so a conflict inside one, which no turning can mend, never spreads out of it.
/// Between patches the seams with the most edges are honoured first. If that leaves
/// more edges in conflict than the file had, the file's own winding stands.
fn settle(faces: usize, links: &[Link]) -> Vec<bool> {
    let mut patches = Components::new(faces);
    for link in links.iter().filter(|link| link.agreed) {
        patches.union(link.faces[0], link.faces[1]);
    }
    let (patch, count) = patches.labels();
    let seams = seams(&patch, links);

    let mut parity = Parity::new(count);
    for seam in &seams {
        parity.join(seam.patches[0], seam.patches[1]);
    }
    let turned = smaller_half(&patch, &mut parity);

    let found: usize = seams.iter().map(|seam| seam.edges).sum();
    let left: usize = seams
        .iter()
        .filter(|seam| turned[seam.patches[0] as usize] == turned[seam.patches[1] as usize])
        .map(|seam| seam.edges)
        .sum();
    if left >= found {
        return vec![false; faces];
    }
    patch.iter().map(|p| turned[*p as usize]).collect()
}

/// Two patches meeting at edges both walk the same way: turning exactly one of them
/// mends all of those edges at once.
struct Seam {
    patches: [u32; 2],
    edges: usize,
}

/// Every seam between two patches, the one with the most edges first.
fn seams(patch: &[u32], links: &[Link]) -> Vec<Seam> {
    let mut pairs: Vec<[u32; 2]> = links
        .iter()
        .filter(|link| !link.agreed)
        .map(|link| link.faces.map(|face| patch[face as usize]))
        .filter(|[a, b]| a != b)
        .map(|[a, b]| [a.min(b), a.max(b)])
        .collect();
    pairs.sort_unstable();
    let mut seams: Vec<Seam> = pairs
        .chunk_by(|a, b| a == b)
        .map(|run| Seam {
            patches: run[0],
            edges: run.len(),
        })
        .collect();
    seams.sort_by_key(|seam| (Reverse(seam.edges), seam.patches));
    seams
}

/// Whether each patch is turned. Within a group of patches the seams tie together only
/// the two opposite assignments matter, so the one turning fewer faces wins: the repair
/// is minimal, and the outward test afterwards decides which way the shell faces.
fn smaller_half(patch: &[u32], parity: &mut Parity) -> Vec<bool> {
    let count = parity.parent.len();
    let resolved: Vec<(u32, bool)> = (0..count as u32).map(|p| parity.find(p)).collect();
    let mut faces = vec![(0usize, 0usize); count];
    for p in patch {
        let (root, turned) = resolved[*p as usize];
        let tally = &mut faces[root as usize];
        tally.0 += usize::from(turned);
        tally.1 += 1;
    }
    resolved
        .iter()
        .map(|&(root, turned)| {
            let (flipped, total) = faces[root as usize];
            turned != (flipped * 2 > total)
        })
        .collect()
}

/// Union-find that also keeps whether each element is turned relative to its root.
struct Parity {
    parent: Vec<u32>,
    turned: Vec<bool>,
}

impl Parity {
    fn new(len: usize) -> Self {
        Self {
            parent: (0..len as u32).collect(),
            turned: vec![false; len],
        }
    }

    fn find(&mut self, node: u32) -> (u32, bool) {
        let (mut root, mut turned) = (node, false);
        while self.parent[root as usize] != root {
            turned ^= self.turned[root as usize];
            root = self.parent[root as usize];
        }
        let (mut at, mut below) = (node, turned);
        while self.parent[at as usize] != at {
            let next = self.parent[at as usize];
            let own = self.turned[at as usize];
            self.parent[at as usize] = root;
            self.turned[at as usize] = below;
            below ^= own;
            at = next;
        }
        (root, turned)
    }

    /// Ties `a` and `b` to opposite turnings, unless they are tied already.
    fn join(&mut self, a: u32, b: u32) {
        let (root_a, turned_a) = self.find(a);
        let (root_b, turned_b) = self.find(b);
        if root_a != root_b {
            self.parent[root_b as usize] = root_a;
            self.turned[root_b as usize] = !(turned_a ^ turned_b);
        }
    }
}

/// Closed shells whose volume came out negative, meaning they are wound inwards.
fn shells_to_invert(
    mesh: &Mesh,
    shell: &[u32],
    shells: usize,
    touches_open_edge: &[bool],
) -> Vec<bool> {
    let mut open = vec![false; shells];
    for (face, is_open) in touches_open_edge.iter().enumerate() {
        if *is_open {
            open[shell[face] as usize] = true;
        }
    }

    let mut volume = vec![0.0 as Scalar; shells];
    for index in 0..mesh.faces.len() {
        if let Some(triangle) = mesh.triangle(index) {
            volume[shell[index] as usize] += triangle.a.dot(triangle.b.cross(triangle.c));
        }
    }

    (0..shells).map(|s| !open[s] && volume[s] < 0.0).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Vec3, signed_volume};

    /// Corner tetrahedron with every face wound outwards. Volume is 1/6.
    fn tetrahedron() -> Mesh {
        Mesh::new(
            vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z],
            vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
        )
    }

    #[test]
    fn an_already_correct_mesh_is_left_alone() {
        let mut mesh = tetrahedron();
        let report = orient_outward(&mut mesh);

        assert!(report.unchanged());
        assert!(report.orientable);
        assert_eq!(report.inverted_shells, 0);
        assert!((signed_volume(&mesh) - 1.0 / 6.0).abs() < 1e-6);
    }

    #[test]
    fn a_single_inverted_face_is_flipped_back() {
        let mut mesh = tetrahedron();
        mesh.faces[2].swap(1, 2);
        let report = orient_outward(&mut mesh);

        assert_eq!(report.flipped_faces, 1);
        assert_eq!(report.inverted_shells, 0);
        assert_eq!(mesh.faces, tetrahedron().faces);
    }

    #[test]
    fn a_wholly_inverted_shell_is_turned_outwards() {
        let mut mesh = tetrahedron();
        for face in &mut mesh.faces {
            face.swap(1, 2);
        }
        let report = orient_outward(&mut mesh);

        assert_eq!(report.inverted_shells, 1);
        assert_eq!(report.flipped_faces, 4);
        assert!(signed_volume(&mesh) > 0.0);
    }

    #[test]
    fn two_neighbouring_inverted_faces_are_turned_back_together() {
        let mut mesh = tetrahedron();
        mesh.faces[2].swap(1, 2);
        mesh.faces[3].swap(1, 2);
        let report = orient_outward(&mut mesh);

        assert!(report.orientable);
        assert_eq!(report.flipped_faces, 2);
        assert_eq!(mesh.faces, tetrahedron().faces);
    }

    /// A band of `segments` quads with a half twist, wound consistently everywhere but at
    /// the seam where its ends meet turned over.
    fn mobius_strip(segments: usize) -> Mesh {
        let mut vertices = Vec::new();
        for at in 0..segments {
            let around = std::f32::consts::TAU * at as f32 / segments as f32;
            let radial = Vec3::new(around.cos(), around.sin(), 0.0);
            let across = radial * (around / 2.0).cos() + Vec3::Z * (around / 2.0).sin();
            vertices.push(radial * 10.0 - across);
            vertices.push(radial * 10.0 + across);
        }
        let mut faces = Vec::new();
        for at in 0..segments as u32 {
            let (inner, outer) = (2 * at, 2 * at + 1);
            let (next_inner, next_outer) = if at + 1 == segments as u32 {
                (1, 0)
            } else {
                (2 * at + 2, 2 * at + 3)
            };
            faces.push([inner, outer, next_outer]);
            faces.push([inner, next_outer, next_inner]);
        }
        Mesh::new(vertices, faces)
    }

    /// No turning mends a half twist; a walk that tried would move the seam round the band
    /// and turn half of it on the way.
    #[test]
    fn a_conflict_no_turning_can_mend_is_left_where_the_file_put_it() {
        let mut mesh = mobius_strip(12);
        let before = crate::diagnose(&mesh).unbalanced_edges;
        let report = orient_outward(&mut mesh);

        assert!(!report.orientable);
        assert!(report.unchanged(), "turned {} faces", report.flipped_faces);
        assert_eq!(crate::diagnose(&mesh).unbalanced_edges, before);
        assert_eq!(mesh.faces, mobius_strip(12).faces);
    }

    #[test]
    fn an_empty_mesh_is_trivially_oriented() {
        let report = orient_outward(&mut Mesh::default());
        assert!(report.unchanged());
        assert!(report.orientable);
    }
}
