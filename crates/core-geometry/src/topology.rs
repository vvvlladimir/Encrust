use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering::Relaxed};

use rayon::iter::{
    IndexedParallelIterator, IntoParallelIterator, IntoParallelRefIterator, ParallelIterator,
};

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

/// Every edge use in the mesh, sorted so uses of the same edge are adjacent. Counted by lower
/// vertex and sorted within runs: linear in the face count, not n log n.
pub(crate) fn edge_uses(mesh: &Mesh) -> Vec<EdgeUse> {
    let buckets = mesh
        .faces
        .par_iter()
        .filter_map(|face| face.iter().max().copied())
        .max()
        .map_or(0, |highest| highest as usize + 1);

    let use_of = |corner: usize| {
        let indices = mesh.faces[corner / 3];
        let from = indices[corner % 3];
        let to = indices[(corner + 1) % 3];
        let forward = from < to;
        EdgeUse {
            edge: if forward { (from, to) } else { (to, from) },
            face: (corner / 3) as u32,
            forward,
        }
    };
    let (order, starts) = counted_out(mesh.faces.len() * 3, buckets, |corner| {
        use_of(corner).edge.0 as usize
    });
    let mut uses: Vec<EdgeUse> = order
        .par_iter()
        .map(|corner| use_of(corner.load(Relaxed) as usize))
        .collect();
    drop(order);

    sort_runs(&mut uses, &starts);
    uses
}

/// The indices `0..len` laid out by `bucket`, lowest bucket first, and where each bucket's
/// run starts: a counting sort on rayon's threads. The order within a run is the threads'.
pub(crate) fn counted_out(
    len: usize,
    buckets: usize,
    bucket: impl Fn(usize) -> usize + Sync,
) -> (Vec<AtomicU32>, Vec<usize>) {
    let counts: Vec<AtomicUsize> = (0..=buckets).map(|_| AtomicUsize::new(0)).collect();
    (0..len).into_par_iter().for_each(|item| {
        counts[bucket(item) + 1].fetch_add(1, Relaxed);
    });
    let mut starts: Vec<usize> = counts.into_iter().map(AtomicUsize::into_inner).collect();
    for at in 1..starts.len() {
        starts[at] += starts[at - 1];
    }

    let cursor: Vec<AtomicUsize> = starts[..buckets]
        .iter()
        .map(|&s| AtomicUsize::new(s))
        .collect();
    let order: Vec<AtomicU32> = (0..len)
        .into_par_iter()
        .map(|_| AtomicU32::new(0))
        .collect();
    (0..len).into_par_iter().for_each(|item| {
        let slot = cursor[bucket(item)].fetch_add(1, Relaxed);
        order[slot].store(item as u32, Relaxed);
    });
    (order, starts)
}

/// Sorts every run of `items` that `starts` bounds, a block of runs to a task on rayon's
/// threads: there are as many runs as vertices, each a handful long.
pub(crate) fn sort_runs<T: Ord + Send>(items: &mut [T], starts: &[usize]) {
    const RUNS_PER_TASK: usize = 4096;
    let runs = starts.len().saturating_sub(1);
    let mut blocks = Vec::with_capacity(runs / RUNS_PER_TASK + 1);
    let mut rest = items;
    for first in (0..runs).step_by(RUNS_PER_TASK) {
        let last = (first + RUNS_PER_TASK).min(runs);
        let (block, tail) = rest.split_at_mut(starts[last] - starts[first]);
        blocks.push((block, &starts[first..=last]));
        rest = tail;
    }
    blocks.into_par_iter().for_each(|(block, bounds)| {
        for run in bounds.windows(2) {
            block[run[0] - bounds[0]..run[1] - bounds[0]].sort_unstable();
        }
    });
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

/// Union-find over indices of faces, vertices or corners that gathers them into the joined groups.
/// Parents are atomic so `joined` can join from every thread; relaxed, they cost a plain integer.
#[derive(Debug)]
pub struct Components {
    parent: Vec<AtomicU32>,
}

impl Clone for Components {
    fn clone(&self) -> Self {
        Self {
            parent: self
                .parent
                .iter()
                .map(|up| AtomicU32::new(up.load(Relaxed)))
                .collect(),
        }
    }
}

impl Components {
    /// `len` indices, each a group of its own.
    pub fn new(len: usize) -> Self {
        Self {
            parent: (0..len as u32).map(AtomicU32::new).collect(),
        }
    }

    /// The index standing for the group `node` is in.
    pub fn find(&mut self, mut node: u32) -> u32 {
        let parent = &mut self.parent;
        loop {
            let up = *parent[node as usize].get_mut();
            if up == node {
                return node;
            }
            let grandparent = *parent[up as usize].get_mut();
            *parent[node as usize].get_mut() = grandparent;
            node = grandparent;
        }
    }

    /// Joins the groups of `a` and `b`.
    pub fn union(&mut self, a: u32, b: u32) {
        let (a, b) = (self.find(a), self.find(b));
        if a != b {
            *self.parent[a.max(b) as usize].get_mut() = a.min(b);
        }
    }

    /// `len` indices grouped as `pairs` join them, on rayon's threads.
    pub(crate) fn joined(len: usize, pairs: impl ParallelIterator<Item = [u32; 2]>) -> Self {
        let parent: Vec<AtomicU32> = (0..len as u32)
            .into_par_iter()
            .map(AtomicU32::new)
            .collect();
        pairs.for_each(|[a, b]| join_toward_lower(&parent, a, b));
        Self { parent }
    }

    /// Dense component label per element, and how many components there are, numbered in
    /// the order of each component's first element.
    pub fn labels(&mut self) -> (Vec<u32>, usize) {
        // Every join hangs the higher root under the lower, so a parent is never above its
        // child and one pass upwards meets each parent's label before the child's.
        let mut labels = vec![0u32; self.parent.len()];
        let mut count = 0;
        for (node, up) in self.parent.iter_mut().enumerate() {
            let up = *up.get_mut();
            labels[node] = if up as usize == node {
                count += 1;
                count - 1
            } else {
                labels[up as usize]
            };
        }
        (labels, count as usize)
    }

    /// How many groups there are.
    pub(crate) fn count(&self) -> usize {
        self.parent
            .par_iter()
            .enumerate()
            .filter(|(node, up)| up.load(Relaxed) as usize == *node)
            .count()
    }
}

/// Joins the trees of `a` and `b` by hanging the higher root under the lower, so that a
/// root is always the least index in its group whatever order the threads ran in.
fn join_toward_lower(parent: &[AtomicU32], mut a: u32, mut b: u32) {
    loop {
        a = root_halving(parent, a);
        b = root_halving(parent, b);
        if a == b {
            return;
        }
        let (low, high) = (a.min(b), a.max(b));
        if parent[high as usize]
            .compare_exchange(high, low, Relaxed, Relaxed)
            .is_ok()
        {
            return;
        }
    }
}

/// The root above `node`, pointing each step at its grandparent on the way up.
fn root_halving(parent: &[AtomicU32], mut node: u32) -> u32 {
    loop {
        let up = parent[node as usize].load(Relaxed);
        if up == node {
            return node;
        }
        let grand = parent[up as usize].load(Relaxed);
        if grand != up {
            let _ = parent[node as usize].compare_exchange(up, grand, Relaxed, Relaxed);
        }
        node = grand;
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
    fn components_joined_on_many_threads_label_as_one_thread_does() {
        let len = 200_000u32;
        let pairs: Vec<[u32; 2]> = (0..len)
            .filter(|at| at % 7 != 0)
            .map(|at| [at, (at * 7919 + 13) % len])
            .collect();
        let mut serial = Components::new(len as usize);
        for [a, b] in &pairs {
            serial.union(*a, *b);
        }

        let mut joined = Components::joined(len as usize, pairs.par_iter().copied());
        assert_eq!(joined.count(), serial.labels().1);
        assert_eq!(joined.labels(), serial.labels());
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
