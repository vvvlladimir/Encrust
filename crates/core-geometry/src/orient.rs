use std::cmp::Reverse;

use rayon::iter::{
    IndexedParallelIterator, IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator,
};
use serde::{Deserialize, Serialize};

mod nesting;

use crate::topology::{Components, agree, edge_groups, edge_uses};
use crate::{Mesh, Scalar};

/// What [`orient_outward`] had to change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Orientation {
    /// Faces whose winding differs from the input.
    pub flipped_faces: usize,
    /// Closed shells turned inside out as a whole: wound inwards with nothing around them,
    /// or inside a shell that was turned.
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

/// Makes every face wind the same way and turns closed shells outwards, a cavity's inwards;
/// STL normals are ignored.
/// Run [`crate::weld`] first; see `docs/design/mesh-repair.md`.
pub fn orient_outward(mesh: &mut Mesh) -> Orientation {
    if mesh.faces.is_empty() {
        return Orientation {
            flipped_faces: 0,
            inverted_shells: 0,
            orientable: true,
        };
    }

    let (links, touches_open_edge) = links(mesh);
    let mut flip = settle(mesh.faces.len(), &links);
    let (shell, shells) =
        Components::joined(mesh.faces.len(), links.par_iter().map(|link| link.faces)).labels();
    let turned = |face: &mut [u32; 3], wanted: bool| {
        if wanted {
            face.swap(1, 2);
        }
    };
    mesh.faces
        .par_iter_mut()
        .zip(&flip)
        .for_each(|(face, wanted)| turned(face, *wanted));
    let invert = shells_to_invert(mesh, &shell, shells, &touches_open_edge);
    let inverted_shells = invert.iter().filter(|wanted| **wanted).count();
    mesh.faces
        .par_iter_mut()
        .zip(&mut flip)
        .zip(&shell)
        .for_each(|((face, flipped), shell)| {
            let wanted = invert[*shell as usize];
            turned(face, wanted);
            *flipped ^= wanted;
        });

    Orientation {
        flipped_faces: flip.par_iter().filter(|flipped| **flipped).count(),
        inverted_shells,
        orientable: links.par_iter().all(|link| {
            let [a, b] = link.faces.map(|face| flip[face as usize]);
            link.agreed == (a == b)
        }),
    }
}

/// Every edge exactly two faces use as a link between them, and for each face whether it
/// has an edge any other number of faces use.
fn links(mesh: &Mesh) -> (Vec<Link>, Vec<bool>) {
    let uses = edge_uses(mesh);
    let mut links = Vec::new();
    let mut open = vec![false; mesh.faces.len()];
    for group in edge_groups(&uses) {
        if let [first, second] = group {
            links.push(Link {
                faces: [first.face, second.face],
                agreed: agree(first, second),
            });
        } else {
            for edge_use in group {
                open[edge_use.face as usize] = true;
            }
        }
    }
    (links, open)
}

/// Two faces sharing an edge no third face uses, and whether they walk it in opposite
/// directions as they stand.
struct Link {
    faces: [u32; 2],
    agreed: bool,
}

/// Agreeing faces form patches turned whole; bigger seams win; the file's winding stands unless
/// beaten.
fn settle(faces: usize, links: &[Link]) -> Vec<bool> {
    let agreed = links
        .par_iter()
        .filter(|link| link.agreed)
        .map(|link| link.faces);
    let (patch, count) = Components::joined(faces, agreed).labels();
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

/// Closed shells to turn inside out: one with nothing around it when it is wound inwards,
/// one inside another whenever that one is turned, so a cavity stays a cavity.
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

    let closed: Vec<bool> = open.iter().map(|open| !open).collect();
    // With every closed shell wound outwards, nesting turns none of them.
    if !(0..shells).any(|s| closed[s] && volume[s] < 0.0) {
        return vec![false; shells];
    }
    let holder = nesting::holders(mesh, shell, &closed, &volume);
    // A holder encloses more than what it holds, so it is decided first.
    let mut order: Vec<usize> = (0..shells).filter(|&s| closed[s]).collect();
    order.sort_by(|a, b| volume[*b].abs().total_cmp(&volume[*a].abs()));
    let mut invert = vec![false; shells];
    for s in order {
        invert[s] = match holder[s] {
            Some(around) => invert[around as usize],
            None => volume[s] < 0.0,
        };
    }
    invert
}

#[cfg(test)]
mod tests;
