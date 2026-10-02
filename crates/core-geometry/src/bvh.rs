use crate::{
    Aabb, ClosestPoint, Mesh, Ray, RayHit, Scalar, Vec3, point_aabb_squared, point_triangle,
    ray_aabb, ray_triangle,
};

/// Faces a leaf may hold. Four is small enough that a leaf costs less than the traversal
/// splitting it would save, and large enough that the tree stays shallow; see
/// `benches/raycast.rs`.
const LEAF_FACES: usize = 4;

/// Deepest the tree can go. A median split halves the faces every level, so this is
/// reached only by a mesh of more faces than memory holds.
const MAX_DEPTH: usize = 64;

/// How many nodes a traversal can have waiting. Popping the deepest internal node, at
/// level `MAX_DEPTH - 1`, leaves `MAX_DEPTH - 1` entries and pushes two more, so the
/// stack needs one slot beyond the depth of the tree and never more.
const STACK: usize = MAX_DEPTH + 1;

/// A bounding volume hierarchy over a mesh's faces, in that mesh's own space.
///
/// Built once per mesh and used for every ray against it. A support run casts one ray per
/// column and a print-sized mesh is a million faces, which is far past what testing every
/// face can carry; see `docs/decisions/0036`.
#[derive(Debug, Clone, Default)]
pub struct Bvh {
    nodes: Vec<Node>,
    /// Face indices, grouped so that every leaf owns one contiguous stretch.
    faces: Vec<u32>,
}

#[derive(Debug, Clone)]
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
            .filter_map(|face| {
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
            })
            .collect();

        let mut nodes = Vec::with_capacity(2 * items.len() / LEAF_FACES + 1);
        if !items.is_empty() {
            split(&mut nodes, &mut items, 0, 0);
        }

        Self {
            faces: items.iter().map(|item| item.face).collect(),
            nodes,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Nearest face of `mesh` that `ray` hits, in the mesh's own space.
    ///
    /// `mesh` must be the one the hierarchy was built from; a different mesh answers with
    /// whatever its faces of the same indices happen to be.
    pub fn raycast(&self, mesh: &Mesh, ray: &Ray) -> Option<RayHit> {
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
            if best.is_some_and(|hit| entry >= hit.t) {
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
            for &face in &self.faces[leaf] {
                let Some(triangle) = mesh.triangle(face as usize) else {
                    continue;
                };
                let Some(t) = ray_triangle(ray, &triangle) else {
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

    /// Nearest point of `mesh` to `point`, in the mesh's own space.
    ///
    /// `mesh` must be the one the hierarchy was built from. A narrow-band distance field
    /// asks this of every voxel near the surface; see `docs/design/distance-queries.md`.
    pub fn closest(&self, mesh: &Mesh, point: Vec3) -> Option<ClosestPoint> {
        let mut best: Option<ClosestPoint> = None;
        let mut best_squared = Scalar::INFINITY;
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
            for &face in &self.faces[leaf] {
                let Some(triangle) = mesh.triangle(face as usize) else {
                    continue;
                };
                let on_face = point_triangle(point, &triangle);
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

    /// Every face of `mesh` whose leaf lies within `radius_mm` of `point`, appended to
    /// `found`.
    ///
    /// Conservative: a leaf is taken whole, so the list holds faces further off as well.
    /// It is what lets a block of voxels share one traversal instead of one each; see
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

/// Builds the subtree covering `items`, which start at `offset` in the final face order,
/// and returns the index of its root node.
///
/// `items` is reordered in place, so a leaf is a contiguous stretch of it.
fn split(nodes: &mut Vec<Node>, items: &mut [Item], offset: usize, depth: usize) -> usize {
    let index = nodes.len();
    nodes.push(Node {
        bounds: bounds_of(items.iter().map(|item| item.bounds)),
        at: offset as u32,
        count: items.len() as u32,
    });

    if items.len() <= LEAF_FACES || depth == MAX_DEPTH {
        return index;
    }

    let centroids = bounds_of(
        items
            .iter()
            .map(|item| Aabb::new(item.centroid, item.centroid)),
    );
    let axis = widest_axis(&centroids);
    // Every face sitting on the same spot cannot be told apart by any split.
    if centroids.maxs[axis] - centroids.mins[axis] <= 0.0 {
        return index;
    }

    let mid = items.len() / 2;
    items.select_nth_unstable_by(mid, |a, b| a.centroid[axis].total_cmp(&b.centroid[axis]));
    let (left, right) = items.split_at_mut(mid);

    nodes[index].count = 0;
    split(nodes, left, offset, depth + 1);
    let far = split(nodes, right, offset + mid, depth + 1);
    nodes[index].at = far as u32;
    index
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
mod tests {
    use super::*;
    use crate::raycast;

    /// What the gathered list is for: a block of voxels around `point` must be able to
    /// answer from it exactly as it would from a traversal of its own.
    #[test]
    fn gathering_a_ball_holds_the_face_the_search_would_have_picked() {
        let mesh = cube(10.0);
        let bvh = Bvh::build(&mesh);
        let mut near = Vec::new();

        for point in [
            Vec3::new(5.0, 5.0, 5.0),
            Vec3::new(-2.0, 5.0, 5.0),
            Vec3::new(11.0, 11.0, 11.0),
        ] {
            let found = bvh.closest(&mesh, point).expect("the cube has faces");
            bvh.faces_within(point, found.distance, &mut near);
            assert!(
                near.contains(&(found.face as u32)),
                "a ball of the nearest distance must hold the nearest face, {point} missed it"
            );
        }
    }

    #[test]
    fn a_ball_past_the_whole_mesh_gathers_every_face() {
        let mesh = cube(10.0);
        let bvh = Bvh::build(&mesh);
        let mut near = Vec::new();
        bvh.faces_within(Vec3::splat(5.0), 100.0, &mut near);
        assert_eq!(near.len(), mesh.faces.len());
    }

    #[test]
    fn a_ball_gathers_nothing_from_an_empty_hierarchy() {
        let mesh = Mesh::default();
        let bvh = Bvh::build(&mesh);
        let mut near = vec![7];
        bvh.faces_within(Vec3::ZERO, 100.0, &mut near);
        assert!(near.is_empty());
    }

    /// A cube from the origin to `(size, size, size)`, twelve triangles.
    fn cube(size: Scalar) -> Mesh {
        let s = size;
        Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(s, 0.0, 0.0),
                Vec3::new(s, s, 0.0),
                Vec3::new(0.0, s, 0.0),
                Vec3::new(0.0, 0.0, s),
                Vec3::new(s, 0.0, s),
                Vec3::new(s, s, s),
                Vec3::new(0.0, s, s),
            ],
            vec![
                [0, 3, 2],
                [0, 2, 1],
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
            ],
        )
    }

    /// A grid of separate triangles in the z = 0 plane, one per cell, so that the tree
    /// has something to split and every ray has one right answer.
    fn tile_field(side: u32) -> Mesh {
        let mut vertices = Vec::new();
        let mut faces = Vec::new();
        for y in 0..side {
            for x in 0..side {
                let base = vertices.len() as u32;
                let (x, y) = (x as Scalar, y as Scalar);
                vertices.push(Vec3::new(x, y, 0.0));
                vertices.push(Vec3::new(x + 0.9, y, 0.0));
                vertices.push(Vec3::new(x, y + 0.9, 0.0));
                faces.push([base, base + 1, base + 2]);
            }
        }
        Mesh::new(vertices, faces)
    }

    #[test]
    fn an_empty_mesh_builds_an_empty_hierarchy() {
        let bvh = Bvh::build(&Mesh::default());
        assert!(bvh.is_empty());
        assert_eq!(
            bvh.raycast(&Mesh::default(), &Ray::new(Vec3::ZERO, Vec3::Z)),
            None
        );
    }

    #[test]
    fn a_ray_down_the_middle_hits_the_top_face() {
        let mesh = cube(10.0);
        let bvh = Bvh::build(&mesh);
        let ray = Ray::new(Vec3::new(5.0, 5.0, 20.0), -Vec3::Z);

        let hit = bvh.raycast(&mesh, &ray).expect("the cube is under the ray");
        assert!(
            (hit.t - 10.0).abs() < 1e-4,
            "the top face is 10 mm below the origin, got {}",
            hit.t
        );
    }

    #[test]
    fn a_ray_that_misses_hits_nothing() {
        let mesh = cube(10.0);
        let bvh = Bvh::build(&mesh);
        let ray = Ray::new(Vec3::new(50.0, 50.0, 20.0), -Vec3::Z);
        assert_eq!(bvh.raycast(&mesh, &ray), None);
    }

    #[test]
    fn every_face_is_still_reachable_after_the_build_reorders_them() {
        let mesh = tile_field(16);
        let bvh = Bvh::build(&mesh);

        for face in 0..mesh.faces.len() {
            let triangle = mesh.triangle(face).expect("the face is in range");
            let centre = (triangle.a + triangle.b + triangle.c) / 3.0;
            let ray = Ray::new(centre + Vec3::Z, -Vec3::Z);
            assert_eq!(
                bvh.raycast(&mesh, &ray).map(|hit| hit.face),
                Some(face),
                "the ray through the middle of face {face} must find it"
            );
        }
    }

    #[test]
    fn the_hierarchy_answers_what_testing_every_face_answers() {
        let mesh = tile_field(12);
        let bvh = Bvh::build(&mesh);

        // Rays over the whole field at an angle, so they cross several leaves each.
        for step in 0..200 {
            let along = step as Scalar * 0.061;
            let origin = Vec3::new(along, along * 0.5, 5.0);
            let ray = Ray::new(origin, Vec3::new(0.2, 0.1, -1.0));
            assert_eq!(
                bvh.raycast(&mesh, &ray),
                raycast(&mesh, &ray),
                "the hierarchy disagreed with a brute force sweep at step {step}"
            );
        }
    }

    #[test]
    fn faces_that_lie_on_one_spot_end_up_in_one_leaf() {
        // Twenty copies of the same triangle: no split can tell them apart, and the build
        // must stop rather than recurse forever.
        let mut vertices = Vec::new();
        let mut faces = Vec::new();
        for _ in 0..20 {
            let base = vertices.len() as u32;
            vertices.push(Vec3::ZERO);
            vertices.push(Vec3::X);
            vertices.push(Vec3::Y);
            faces.push([base, base + 1, base + 2]);
        }
        let mesh = Mesh::new(vertices, faces);
        let bvh = Bvh::build(&mesh);

        let ray = Ray::new(Vec3::new(0.2, 0.2, 1.0), -Vec3::Z);
        assert!(bvh.raycast(&mesh, &ray).is_some());
    }

    #[test]
    fn the_hierarchy_finds_the_same_nearest_point_as_every_face() {
        let mesh = cube(10.0);
        let bvh = Bvh::build(&mesh);
        for point in [
            Vec3::new(-3.0, 5.0, 5.0),
            Vec3::new(5.0, 5.0, 5.0),
            Vec3::new(12.0, 13.0, 14.0),
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(5.0, 5.0, 9.9),
        ] {
            let exact = crate::closest_point(&mesh, point).expect("the cube has faces");
            let found = bvh.closest(&mesh, point).expect("the cube has faces");
            assert!(
                (found.distance - exact.distance).abs() < 1e-5,
                "at {point}: every face gives {}, the hierarchy {}",
                exact.distance,
                found.distance
            );
        }
    }

    #[test]
    fn a_point_inside_a_cube_is_nearest_to_the_wall_it_stands_closest_to() {
        let mesh = cube(10.0);
        let bvh = Bvh::build(&mesh);
        let found = bvh
            .closest(&mesh, Vec3::new(2.0, 5.0, 5.0))
            .expect("the cube has faces");
        assert!(
            (found.distance - 2.0).abs() < 1e-5,
            "got {}",
            found.distance
        );
        assert!((found.point - Vec3::new(0.0, 5.0, 5.0)).length() < 1e-5);
    }

    #[test]
    fn an_empty_hierarchy_has_no_nearest_point() {
        let mesh = Mesh::default();
        assert!(Bvh::build(&mesh).closest(&mesh, Vec3::ZERO).is_none());
    }
}
