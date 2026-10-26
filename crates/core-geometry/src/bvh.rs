use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};

use crate::{
    Aabb, ClosestPoint, Mesh, Ray, RayHit, Scalar, Triangle, Vec3, point_aabb_squared,
    point_triangle, ray_aabb, ray_triangle,
};

/// Faces a leaf may hold. Four is small enough that a leaf costs less than the traversal
/// splitting it would save, and large enough that the tree stays shallow; see
/// `benches/raycast.rs`.
const LEAF_FACES: usize = 4;

/// Fewest faces a subtree needs before its halves are built on two threads: below it the
/// hand-over costs more than the half it saves; see `benches/raycast.rs`.
const PARALLEL_FACES: usize = 16_384;

/// Deepest the tree can go. A median split halves the faces every level, so this is
/// reached only by a mesh of more faces than memory holds.
const MAX_DEPTH: usize = 64;

/// How many nodes a traversal can have waiting. Popping the deepest internal node, at
/// level `MAX_DEPTH - 1`, leaves `MAX_DEPTH - 1` entries and pushes two more, so the
/// stack needs one slot beyond the depth of the tree and never more.
const STACK: usize = MAX_DEPTH + 1;

/// Built once per mesh, holds its own triangles (ADR 0036, `docs/design/distance-queries.md`).
#[derive(Debug, Clone, Default)]
pub struct Bvh {
    nodes: Vec<Node>,
    /// Face indices, grouped so that every leaf owns one contiguous stretch.
    faces: Vec<u32>,
    /// The triangle of each entry of `faces`, beside it in memory.
    triangles: Vec<Triangle>,
}

#[derive(Debug, Clone, PartialEq)]
struct Node {
    bounds: Aabb,
    /// First face of a leaf, or the right child of an inner node.
    at: u32,
    /// Faces in a leaf. Zero marks an inner node, whose left child is the next node.
    count: u32,
}

/// One face as the build sees it: where it is and how big it is.
struct Item {
    face: u32,
    centroid: Vec3,
    bounds: Aabb,
}

impl Bvh {
    /// Builds the hierarchy of `mesh`. Degenerate faces are kept: they are still hit by a
    /// ray through their edge, and dropping them would shift every face index.
    pub fn build(mesh: &Mesh) -> Self {
        let mut items: Vec<Item> = (0..mesh.faces.len())
            .into_par_iter()
            .filter_map(|face| item(mesh, face))
            .collect();

        let nodes = if items.is_empty() {
            Vec::new()
        } else {
            subtree(&mut items, 0, 0)
        };

        let faces: Vec<u32> = items.iter().map(|item| item.face).collect();
        drop(items);
        // Every face kept by the build names vertices that are there.
        let triangles = faces
            .par_iter()
            .filter_map(|&face| mesh.triangle(face as usize))
            .collect();
        Self {
            nodes,
            faces,
            triangles,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Nearest face that `ray` hits, in the space of the mesh the hierarchy was built from.
    pub fn raycast(&self, ray: &Ray) -> Option<RayHit> {
        self.raycast_within(ray, Scalar::INFINITY)
    }

    /// [`Self::raycast`] looking no further than `max_t` along the ray: a short look skips
    /// every box beyond it rather than walking the whole line through the mesh.
    pub fn raycast_within(&self, ray: &Ray, max_t: Scalar) -> Option<RayHit> {
        let mut best: Option<RayHit> = None;
        // An array rather than a `Vec`, so that a cast costs no allocation; a support run
        // makes one of these per column.
        let mut stack = [0u32; STACK];
        let mut depth = usize::from(!self.nodes.is_empty());

        while depth > 0 {
            depth -= 1;
            let index = stack[depth];
            let node = &self.nodes[index as usize];
            let Some(entry) = ray_aabb(ray, &node.bounds) else {
                continue;
            };
            // Everything in this box is further away than what has already been hit.
            if entry > max_t || best.is_some_and(|hit| entry >= hit.t) {
                continue;
            }

            if node.count == 0 {
                // The left child is next in the array, and goes on last so it is looked
                // at first: it is the nearer half more often than not.
                stack[depth] = node.at;
                stack[depth + 1] = index + 1;
                depth += 2;
                continue;
            }

            let leaf = node.at as usize..(node.at + node.count) as usize;
            for (&face, triangle) in self.faces[leaf.clone()].iter().zip(&self.triangles[leaf]) {
                let Some(t) = ray_triangle(ray, triangle).filter(|t| *t <= max_t) else {
                    continue;
                };
                if best.is_none_or(|hit| t < hit.t) {
                    best = Some(RayHit {
                        t,
                        face: face as usize,
                    });
                }
            }
        }

        best
    }

    /// Nearest point of the mesh the hierarchy was built from to `point`, in its own space.
    ///
    /// A narrow-band distance field asks this of every voxel near the surface; see
    /// `docs/design/distance-queries.md`.
    pub fn closest(&self, point: Vec3) -> Option<ClosestPoint> {
        self.closest_within(point, Scalar::INFINITY)
    }

    /// Nearest point to `point` that is nearer than `within_mm`, or `None` when none is.
    ///
    /// A caller that already knows a face at `within_mm` gets the exact answer for far
    /// less: every node no nearer than that is culled before it is opened.
    pub fn closest_within(&self, point: Vec3, within_mm: Scalar) -> Option<ClosestPoint> {
        self.closest_kept(point, within_mm, |_| true)
    }

    /// Nearest point to `point` on a face `keep` answers true for, or `None` when there is
    /// none: one side of a wall, say, whichever is nearer.
    pub fn closest_where(&self, point: Vec3, keep: impl Fn(usize) -> bool) -> Option<ClosestPoint> {
        self.closest_kept(point, Scalar::INFINITY, keep)
    }

    fn closest_kept(
        &self,
        point: Vec3,
        within_mm: Scalar,
        keep: impl Fn(usize) -> bool,
    ) -> Option<ClosestPoint> {
        let mut best: Option<ClosestPoint> = None;
        let mut best_squared = within_mm * within_mm;
        // Each entry carries the distance its box was at when it was pushed, so a node
        // the search has since outgrown is dropped without measuring its box again. The
        // stack is an array rather than a `Vec`: a field build asks this of millions of
        // voxels, and one allocation each is most of the query.
        let mut stack = [(0u32, 0.0 as Scalar); STACK];
        let mut depth = usize::from(!self.nodes.is_empty());

        while depth > 0 {
            depth -= 1;
            let (index, squared) = stack[depth];
            if squared >= best_squared {
                continue;
            }
            let node = &self.nodes[index as usize];

            if node.count == 0 {
                let (left, right) = (index + 1, node.at);
                let near_left = point_aabb_squared(point, &self.nodes[left as usize].bounds);
                let near_right = point_aabb_squared(point, &self.nodes[right as usize].bounds);
                // The nearer child goes on last so it is looked at first: it tightens the
                // bound the other one is then culled against.
                let (far, near) = if near_left <= near_right {
                    ((right, near_right), (left, near_left))
                } else {
                    ((left, near_left), (right, near_right))
                };
                stack[depth] = far;
                stack[depth + 1] = near;
                depth += 2;
                continue;
            }

            let leaf = node.at as usize..(node.at + node.count) as usize;
            for (&face, triangle) in self.faces[leaf.clone()].iter().zip(&self.triangles[leaf]) {
                if !keep(face as usize) {
                    continue;
                }
                let on_face = point_triangle(point, triangle);
                let squared = (on_face - point).length_squared();
                if squared < best_squared {
                    best_squared = squared;
                    best = Some(ClosestPoint {
                        point: on_face,
                        distance: squared.sqrt(),
                        face: face as usize,
                    });
                }
            }
        }

        best
    }

    /// Conservative, since leaves are taken whole; lets voxel blocks share a traversal. See
    /// `docs/design/volume.md`.
    pub fn faces_within(&self, point: Vec3, radius_mm: Scalar, found: &mut Vec<u32>) {
        found.clear();
        let reach_squared = radius_mm * radius_mm;
        let mut stack = [0u32; STACK];
        let mut depth = usize::from(!self.nodes.is_empty());

        while depth > 0 {
            depth -= 1;
            let node = &self.nodes[stack[depth] as usize];
            if point_aabb_squared(point, &node.bounds) > reach_squared {
                continue;
            }
            if node.count == 0 {
                stack[depth] += 1;
                stack[depth + 1] = node.at;
                depth += 2;
                continue;
            }
            let leaf = node.at as usize..(node.at + node.count) as usize;
            found.extend_from_slice(&self.faces[leaf]);
        }
    }
}

impl Bvh {
    /// Every face whose bounds meet `bounds`, conservative as leaves are taken whole.
    pub fn faces_meeting(&self, bounds: &Aabb, found: &mut Vec<u32>) {
        found.clear();
        let mut stack = [0u32; STACK];
        let mut depth = usize::from(!self.nodes.is_empty());

        while depth > 0 {
            depth -= 1;
            let node = &self.nodes[stack[depth] as usize];
            let apart = (node.bounds.mins - bounds.maxs).max_element() > 0.0
                || (bounds.mins - node.bounds.maxs).max_element() > 0.0;
            if apart {
                continue;
            }
            if node.count == 0 {
                stack[depth] += 1;
                stack[depth + 1] = node.at;
                depth += 2;
                continue;
            }
            let leaf = node.at as usize..(node.at + node.count) as usize;
            found.extend_from_slice(&self.faces[leaf]);
        }
    }
}

/// One face as the build sees it, or `None` for a face naming a vertex that is not there.
fn item(mesh: &Mesh, face: usize) -> Option<Item> {
    let triangle = mesh.triangle(face)?;
    let bounds = Aabb::new(
        triangle.a.min(triangle.b).min(triangle.c),
        triangle.a.max(triangle.b).max(triangle.c),
    );
    Some(Item {
        face: face as u32,
        centroid: (bounds.mins + bounds.maxs) * 0.5,
        bounds,
    })
}

/// The nodes over `items`, its root first, with its own children counted from it. A
/// subtree large enough is built with its two halves side by side on rayon's threads and
/// joined after; the nodes come out exactly as one thread would have laid them.
fn subtree(items: &mut [Item], offset: usize, depth: usize) -> Vec<Node> {
    if items.len() < PARALLEL_FACES {
        let mut nodes = Vec::with_capacity(2 * items.len() / LEAF_FACES + 1);
        split(&mut nodes, items, offset, depth);
        return nodes;
    }
    let mut root = leaf(items, offset);
    let Some(mid) = halve(items, depth) else {
        return vec![root];
    };
    let (left, right) = items.split_at_mut(mid);
    let (left, right) = rayon::join(
        || subtree(left, offset, depth + 1),
        || subtree(right, offset + mid, depth + 1),
    );

    let far = 1 + left.len();
    root.count = 0;
    root.at = far as u32;
    let mut nodes = Vec::with_capacity(far + right.len());
    nodes.push(root);
    nodes.extend(shifted(left, 1));
    nodes.extend(shifted(right, far));
    nodes
}

/// A subtree's nodes moved `by` places along the array: an inner node's right child moves
/// with it, a leaf's faces do not.
fn shifted(nodes: Vec<Node>, by: usize) -> impl Iterator<Item = Node> {
    nodes.into_iter().map(move |mut node| {
        if node.count == 0 {
            node.at += by as u32;
        }
        node
    })
}

/// Builds the subtree covering `items`, which start at `offset` in the final face order,
/// and returns the index of its root node.
///
/// `items` is reordered in place, so a leaf is a contiguous stretch of it.
fn split(nodes: &mut Vec<Node>, items: &mut [Item], offset: usize, depth: usize) -> usize {
    let index = nodes.len();
    nodes.push(leaf(items, offset));
    let Some(mid) = halve(items, depth) else {
        return index;
    };
    let (left, right) = items.split_at_mut(mid);

    nodes[index].count = 0;
    split(nodes, left, offset, depth + 1);
    let far = split(nodes, right, offset + mid, depth + 1);
    nodes[index].at = far as u32;
    index
}

/// A leaf over `items`, the faces from `offset` on.
fn leaf(items: &[Item], offset: usize) -> Node {
    Node {
        bounds: bounds_of(items.iter().map(|item| item.bounds)),
        at: offset as u32,
        count: items.len() as u32,
    }
}

/// Orders `items` about the median of their widest axis and returns where the halves
/// meet, or `None` when they stay a leaf.
fn halve(items: &mut [Item], depth: usize) -> Option<usize> {
    if items.len() <= LEAF_FACES || depth == MAX_DEPTH {
        return None;
    }
    let centroids = bounds_of(
        items
            .iter()
            .map(|item| Aabb::new(item.centroid, item.centroid)),
    );
    let axis = widest_axis(&centroids);
    // Every face sitting on the same spot cannot be told apart by any split.
    if centroids.maxs[axis] - centroids.mins[axis] <= 0.0 {
        return None;
    }
    let mid = items.len() / 2;
    items.select_nth_unstable_by(mid, |a, b| a.centroid[axis].total_cmp(&b.centroid[axis]));
    Some(mid)
}

fn bounds_of(boxes: impl IntoIterator<Item = Aabb>) -> Aabb {
    let mut mins = Vec3::splat(Scalar::INFINITY);
    let mut maxs = Vec3::splat(Scalar::NEG_INFINITY);
    for bounds in boxes {
        mins = mins.min(bounds.mins);
        maxs = maxs.max(bounds.maxs);
    }
    Aabb::new(mins, maxs)
}

fn widest_axis(bounds: &Aabb) -> usize {
    let extent = bounds.maxs - bounds.mins;
    if extent.x >= extent.y && extent.x >= extent.z {
        0
    } else if extent.y >= extent.z {
        1
    } else {
        2
    }
}

#[cfg(test)]
mod tests;
