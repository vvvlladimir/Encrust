use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

use super::{Face, SurfacePathError, SurfacePoint, WalkWatch, shared};
use crate::{Adjacency, FastMap, Mat4, Mesh, Scalar, Vec3};

/// How many points each edge offers a walk to cross it at, evenly spaced: the midpoint
/// alone strays 10 % on a coarse ball, three keep the corridor round the shortest walk.
const CROSSINGS: u32 = 3;

/// How many nodes A* expands between two words to the watcher.
const TELL_EVERY: usize = 4096;

/// A face as entered across one point of one of its edges, or the first face as the walk
/// starts: the face, which corner the edge is opposite (3 for the start), and the point.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Node(u64);

impl Node {
    fn start(face: usize) -> Self {
        Self((face as u64) << 4 | 3 << 2)
    }

    fn entered(face: usize, opposite: usize, slot: u32) -> Self {
        Self((face as u64) << 4 | (opposite as u64) << 2 | u64::from(slot))
    }

    fn face(self) -> usize {
        (self.0 >> 4) as usize
    }

    /// The corner of its face the edge it was entered by is opposite; 3 for the start.
    fn opposite(self) -> usize {
        (self.0 >> 2 & 3) as usize
    }
}

/// How a node was reached at its lowest cost.
struct Reached {
    cost_mm: Scalar,
    entry: Vec3,
    parent: Option<Node>,
}

/// A node waiting to be expanded, ordered by its estimate, the further come first on a tie.
struct Queued {
    estimate_mm: Scalar,
    cost_mm: Scalar,
    node: Node,
}

impl PartialEq for Queued {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Queued {}

impl Ord for Queued {
    fn cmp(&self, other: &Self) -> Ordering {
        self.estimate_mm
            .total_cmp(&other.estimate_mm)
            .then(other.cost_mm.total_cmp(&self.cost_mm))
    }
}

impl PartialOrd for Queued {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// The faces from `from` to `to`, each sharing an edge with the next, by A* between points
/// on the edges; it visits at most each face of their shell `CROSSINGS` times per edge.
pub(super) fn find(
    (mesh, placement): (&Mesh, Mat4),
    adjacency: &Adjacency,
    (from, to): (SurfacePoint, SurfacePoint),
    watch: WalkWatch,
) -> Result<Vec<Face>, SurfacePathError> {
    let unreachable = SurfacePathError::Unreachable {
        from: from.face,
        to: to.face,
    };
    let start = Node::start(from.face);
    let first = Reached {
        cost_mm: 0.0,
        entry: from.point,
        parent: None,
    };
    let mut reached = FastMap::from_iter([(start, first)]);
    let straight_mm = from.point.distance(to.point);
    let mut queue = BinaryHeap::from([Reverse(Queued {
        estimate_mm: straight_mm,
        cost_mm: 0.0,
        node: start,
    })]);
    let (mut expanded, mut nearest_mm) = (0usize, straight_mm);
    while let Some(Reverse(Queued { cost_mm, node, .. })) = queue.pop() {
        if node.face() == to.face {
            return walk_back(mesh, placement, &reached, node).ok_or(unreachable);
        }
        let Some(here) = reached.get(&node) else {
            continue;
        };
        if cost_mm > here.cost_mm {
            continue;
        }
        let entry = here.entry;
        nearest_mm = nearest_mm.min(entry.distance(to.point));
        if expanded % TELL_EVERY == 0 && watch(share(nearest_mm, straight_mm)).is_break() {
            return Err(SurfacePathError::Stopped);
        }
        expanded += 1;
        let ids = mesh.faces.get(node.face()).copied().ok_or(unreachable)?;
        let ahead = adjacency.neighbours(node.face()).iter().filter_map(|next| {
            crossings((mesh, placement), (ids, node.opposite()), *next as usize)
        });
        for (next, point) in ahead.flatten() {
            let cost_mm = cost_mm + entry.distance(point);
            if reached
                .get(&next)
                .is_some_and(|seen: &Reached| seen.cost_mm <= cost_mm)
            {
                continue;
            }
            let parent = Some(node);
            reached.insert(
                next,
                Reached {
                    cost_mm,
                    entry: point,
                    parent,
                },
            );
            queue.push(Reverse(Queued {
                estimate_mm: cost_mm + point.distance(to.point),
                cost_mm,
                node: next,
            }));
        }
    }
    Err(unreachable)
}

/// The share of the way found, by how near the end the search has come in a straight line.
fn share(nearest_mm: Scalar, straight_mm: Scalar) -> Scalar {
    if straight_mm > 0.0 {
        (1.0 - nearest_mm / straight_mm).clamp(0.0, 1.0)
    } else {
        1.0
    }
}

/// The points of the edge face `ids`, entered across the edge opposite its corner
/// `entered`, shares with face `next` a walk may cross at, each as the node it enters; none
/// back across the edge it was entered by.
fn crossings(
    (mesh, placement): (&Mesh, Mat4),
    (ids, entered): ([u32; 3], usize),
    next: usize,
) -> Option<[(Node, Vec3); CROSSINGS as usize]> {
    let across = *mesh.faces.get(next)?;
    let shared = shared(ids, across)?;
    if ids.get(entered) == Some(&shared.apex_of_first) {
        return None;
    }
    let opposite = across.iter().position(|id| *id == shared.apex_of_second)?;
    let placed = |id: u32| Some(placement.transform_point3(*mesh.vertices.get(id as usize)?));
    let (low, high) = (
        placed(shared.edge[0].min(shared.edge[1]))?,
        placed(shared.edge[0].max(shared.edge[1]))?,
    );
    Some(std::array::from_fn(|index| {
        let slot = index as u32 + 1;
        let point = low.lerp(high, slot as Scalar / (CROSSINGS + 1) as Scalar);
        (Node::entered(next, opposite, slot), point)
    }))
}

/// The corridor ending at `last`, first face first.
fn walk_back(
    mesh: &Mesh,
    placement: Mat4,
    reached: &FastMap<Node, Reached>,
    last: Node,
) -> Option<Vec<Face>> {
    let mut faces = vec![Face::placed(mesh, placement, last.face())?];
    let mut at = reached.get(&last)?.parent;
    while let Some(node) = at {
        faces.push(Face::placed(mesh, placement, node.face())?);
        at = reached.get(&node)?.parent;
    }
    faces.reverse();
    Some(faces)
}
