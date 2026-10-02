use crate::Mesh;

/// One face's use of one undirected edge.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct EdgeUse {
    /// Vertex indices, lower first, so that both faces sharing an edge produce one key.
    pub edge: (u32, u32),
    pub face: u32,
    /// True when this face walks the edge from the lower index to the higher one.
    pub forward: bool,
}

/// Every edge use in the mesh, sorted so uses of the same edge are adjacent.
///
/// Sorted by counting the uses of each lower vertex first and ordering only within each
/// of those runs, which is the same order a full sort gives and linear in the face count
/// rather than n log n. Both mesh checks were spending nearly all their time in that sort.
pub(crate) fn edge_uses(mesh: &Mesh) -> Vec<EdgeUse> {
    let buckets = mesh
        .faces
        .iter()
        .flatten()
        .copied()
        .max()
        .map_or(0, |highest| highest as usize + 1);

    let mut starts = vec![0u32; buckets + 1];
    for face in &mesh.faces {
        for corner in 0..3 {
            let low = face[corner].min(face[(corner + 1) % 3]) as usize;
            starts[low + 1] += 1;
        }
    }
    for at in 1..starts.len() {
        starts[at] += starts[at - 1];
    }

    let mut uses = vec![EdgeUse::default(); mesh.faces.len() * 3];
    let mut cursor = starts.clone();
    for (face, indices) in mesh.faces.iter().enumerate() {
        for corner in 0..3 {
            let from = indices[corner];
            let to = indices[(corner + 1) % 3];
            let forward = from < to;
            let edge = if forward { (from, to) } else { (to, from) };

            let slot = &mut cursor[edge.0 as usize];
            uses[*slot as usize] = EdgeUse {
                edge,
                face: face as u32,
                forward,
            };
            *slot += 1;
        }
    }

    for run in starts.windows(2) {
        uses[run[0] as usize..run[1] as usize].sort_unstable();
    }
    uses
}

/// Groups a sorted [`edge_uses`] result into one slice per distinct edge.
pub(crate) fn edge_groups(uses: &[EdgeUse]) -> impl Iterator<Item = &[EdgeUse]> {
    uses.chunk_by(|a, b| a.edge == b.edge)
}

/// Two faces sharing an edge agree on orientation when they walk it in opposite
/// directions. Equal directions mean one of them is inside out.
pub(crate) fn agree(a: &EdgeUse, b: &EdgeUse) -> bool {
    a.forward != b.forward
}

/// Which faces of a mesh share an edge with which, for walking over its surface.
///
/// Stored as one flat list of neighbours with a start per face, so a walk reads a slice
/// rather than a map. An edge used by more than two faces makes all of them neighbours.
pub struct Adjacency {
    starts: Vec<u32>,
    neighbours: Vec<u32>,
}

impl Adjacency {
    pub fn of(mesh: &Mesh) -> Self {
        let uses = edge_uses(mesh);
        let mut starts = vec![0u32; mesh.faces.len() + 1];
        for group in edge_groups(&uses) {
            for edge_use in group {
                starts[edge_use.face as usize + 1] += group.len() as u32 - 1;
            }
        }
        for at in 1..starts.len() {
            starts[at] += starts[at - 1];
        }

        let mut neighbours = vec![0u32; starts[mesh.faces.len()] as usize];
        let mut cursor = starts.clone();
        for group in edge_groups(&uses) {
            for edge_use in group {
                for other in group.iter().filter(|other| other.face != edge_use.face) {
                    let slot = &mut cursor[edge_use.face as usize];
                    neighbours[*slot as usize] = other.face;
                    *slot += 1;
                }
            }
        }

        Self { starts, neighbours }
    }

    /// The faces sharing an edge with `face`, empty for a face out of range.
    pub fn neighbours(&self, face: usize) -> &[u32] {
        let Some(&end) = self.starts.get(face + 1) else {
            return &[];
        };
        &self.neighbours[self.starts[face] as usize..end as usize]
    }
}

/// Union-find over face indices, used to group faces into connected shells.
pub(crate) struct Components {
    parent: Vec<u32>,
}

impl Components {
    pub fn new(len: usize) -> Self {
        Self {
            parent: (0..len as u32).collect(),
        }
    }

    pub fn find(&mut self, mut node: u32) -> u32 {
        while self.parent[node as usize] != node {
            let grandparent = self.parent[self.parent[node as usize] as usize];
            self.parent[node as usize] = grandparent;
            node = grandparent;
        }
        node
    }

    pub fn union(&mut self, a: u32, b: u32) {
        let (a, b) = (self.find(a), self.find(b));
        if a != b {
            self.parent[b as usize] = a;
        }
    }

    /// Dense component label per element, and how many components there are.
    pub fn labels(&mut self) -> (Vec<u32>, usize) {
        let mut dense = vec![u32::MAX; self.parent.len()];
        let mut labels = Vec::with_capacity(self.parent.len());
        let mut count = 0;
        for node in 0..self.parent.len() as u32 {
            let root = self.find(node) as usize;
            if dense[root] == u32::MAX {
                dense[root] = count;
                count += 1;
            }
            labels.push(dense[root]);
        }
        (labels, count as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Vec3;

    #[test]
    fn a_shared_edge_produces_one_group_of_two_uses() {
        let mesh = Mesh::new(
            vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::new(1.0, 1.0, 0.0)],
            vec![[0, 1, 2], [1, 3, 2]],
        );
        let uses = edge_uses(&mesh);
        let sizes: Vec<usize> = edge_groups(&uses).map(<[EdgeUse]>::len).collect();

        assert_eq!(uses.len(), 6);
        assert_eq!(sizes.iter().filter(|&&n| n == 2).count(), 1);
        assert_eq!(sizes.iter().filter(|&&n| n == 1).count(), 4);
    }

    #[test]
    fn consistently_wound_neighbours_agree() {
        let mesh = Mesh::new(
            vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::new(1.0, 1.0, 0.0)],
            vec![[0, 1, 2], [1, 3, 2]],
        );
        let uses = edge_uses(&mesh);
        let shared = edge_groups(&uses)
            .find(|g| g.len() == 2)
            .expect("one shared edge");
        assert!(agree(&shared[0], &shared[1]));
    }

    #[test]
    fn components_merge_transitively() {
        let mut components = Components::new(4);
        components.union(0, 1);
        components.union(2, 3);
        let (labels, count) = components.labels();

        assert_eq!(count, 2);
        assert_eq!(labels[0], labels[1]);
        assert_eq!(labels[2], labels[3]);
        assert_ne!(labels[0], labels[2]);
    }

    #[test]
    fn two_triangles_sharing_an_edge_are_neighbours() {
        let mesh = Mesh::new(
            vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::new(1.0, 1.0, 0.0)],
            vec![[0, 1, 2], [1, 3, 2]],
        );
        let adjacency = Adjacency::of(&mesh);

        assert_eq!(adjacency.neighbours(0), &[1]);
        assert_eq!(adjacency.neighbours(1), &[0]);
        assert!(adjacency.neighbours(2).is_empty(), "there is no third face");
    }

    #[test]
    fn a_triangle_alone_has_no_neighbours() {
        let mesh = Mesh::new(vec![Vec3::ZERO, Vec3::X, Vec3::Y], vec![[0, 1, 2]]);
        assert!(Adjacency::of(&mesh).neighbours(0).is_empty());
    }
}
