use crate::{Mesh, Scalar, Triangle, Vec3};

/// Faces a leaf may hold. Larger than the hierarchy's four, because a leaf here is
/// summarised by one dipole rather than walked, so bigger leaves are cheaper as well as
/// shallower.
const LEAF_FACES: usize = 8;

/// Deepest the tree can go, as in `bvh.rs`.
const MAX_DEPTH: usize = 64;

/// How many node radii a query point must stand off before the node is summarised instead
/// of walked. Barill et al. call this beta.
///
/// The dipole term's error falls as the square of the stand-off, and the cost roughly
/// doubles with it: on a 200k-triangle sphere the worst error over a sample of points is
/// 3.6% at two radii and 1.1% at three, for 7.3 and 15.6 microseconds a query. Two is
/// Barill's own recommendation and is what the half-winding this is read against needs;
/// a consumer wanting the winding number itself, rather than its sign, should buy the
/// accuracy with the quadrupole term rather than a wider beta.
const FAR_ENOUGH: Scalar = 2.0;

const FOUR_PI: Scalar = 4.0 * std::f32::consts::PI;

/// Signed solid angle in steradians that `triangle` subtends at `point`, by the formula of
/// Van Oosterom and Strackee (1983).
///
/// Positive when the triangle is wound counter-clockwise seen from `point`, so a closed
/// outward-wound surface sums to `4pi` around a point inside it. Zero when `point` lies on
/// a vertex, which is the only place the formula has nothing to say.
pub fn solid_angle(point: Vec3, triangle: &Triangle) -> Scalar {
    let a = triangle.a - point;
    let b = triangle.b - point;
    let c = triangle.c - point;
    let (la, lb, lc) = (a.length(), b.length(), c.length());

    let numerator = a.dot(b.cross(c));
    let denominator = la * lb * lc + a.dot(b) * lc + b.dot(c) * la + c.dot(a) * lb;
    2.0 * numerator.atan2(denominator)
}

/// Generalised winding number of `mesh` at `point`, by summing every face.
///
/// One inside a closed outward-wound mesh, zero outside it, and a fraction near a hole or
/// a self-intersection — which is what makes it a usable sign on an imported mesh; see
/// `docs/decisions/0054-inside-is-decided-by-the-generalised-winding-number.md`. Linear in the face count: for more than a handful of points,
/// build a [`Winding`] instead, which is what this is measured and tested against.
pub fn winding_number(mesh: &Mesh, point: Vec3) -> Scalar {
    mesh.triangles()
        .map(|triangle| solid_angle(point, &triangle))
        .sum::<Scalar>()
        / FOUR_PI
}

/// A hierarchy over a mesh's faces that answers the generalised winding number in
/// logarithmic time, after Barill et al., *Fast Winding Numbers for Soups and Clouds*
/// (2018).
///
/// Built once per mesh, beside its [`crate::Bvh`], and asked for the sign of every voxel
/// of a distance field. The expansion and what it costs are in
/// `docs/design/distance-queries.md`.
#[derive(Debug, Clone, Default)]
pub struct Winding {
    nodes: Vec<Node>,
    /// Face indices, grouped so that every leaf owns one contiguous stretch.
    faces: Vec<u32>,
}

#[derive(Debug, Clone)]
struct Node {
    /// Point the node's faces are summarised at, weighted by their areas.
    center: Vec3,
    /// Covers every vertex of the node's faces, in millimetres.
    radius: Scalar,
    /// Sum of the node's face normals, each scaled by its own area.
    dipole: Vec3,
    /// First face of a leaf, or the right child of an inner node.
    at: u32,
    /// Faces in a leaf. Zero marks an inner node, whose left child is the next node.
    count: u32,
}

/// One face as the build sees it, summarised so that the build never touches the mesh
/// twice.
struct Item {
    face: u32,
    centroid: Vec3,
    /// Distance from `centroid` to the face's furthest vertex.
    radius: Scalar,
    /// The face normal scaled by its area, which is half its unnormalised normal.
    dipole: Vec3,
    area: Scalar,
}

impl Winding {
    /// Builds the hierarchy of `mesh`. Faces with no area contribute nothing and are kept
    /// so that face indices still line up with the mesh's own.
    pub fn build(mesh: &Mesh) -> Self {
        let mut items: Vec<Item> = (0..mesh.faces.len())
            .filter_map(|face| {
                let triangle = mesh.triangle(face)?;
                let centroid = (triangle.a + triangle.b + triangle.c) / 3.0;
                let dipole = triangle.normal_unnormalized() * 0.5;
                Some(Item {
                    face: face as u32,
                    centroid,
                    radius: (triangle.a - centroid)
                        .length()
                        .max((triangle.b - centroid).length())
                        .max((triangle.c - centroid).length()),
                    area: dipole.length(),
                    dipole,
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

    /// Generalised winding number of `mesh` at `point`: one well inside it, zero well
    /// outside.
    ///
    /// `mesh` must be the one the hierarchy was built from. Faces near `point` are summed
    /// exactly and distant groups are summarised by their dipole, which costs a few
    /// percent of winding at the stand-off of two radii this uses.
    pub fn at(&self, mesh: &Mesh, point: Vec3) -> Scalar {
        let mut total: Scalar = 0.0;
        let mut stack: Vec<u32> = Vec::with_capacity(MAX_DEPTH);
        if !self.nodes.is_empty() {
            stack.push(0);
        }

        while let Some(index) = stack.pop() {
            let node = &self.nodes[index as usize];
            let offset = node.center - point;
            let distance = offset.length();

            if distance > FAR_ENOUGH * node.radius {
                total += offset.dot(node.dipole) / (distance * distance * distance);
                continue;
            }

            if node.count == 0 {
                stack.push(node.at);
                stack.push(index + 1);
                continue;
            }

            let leaf = node.at as usize..(node.at + node.count) as usize;
            for &face in &self.faces[leaf] {
                if let Some(triangle) = mesh.triangle(face as usize) {
                    total += solid_angle(point, &triangle);
                }
            }
        }

        total / FOUR_PI
    }

    /// Whether `point` is inside `mesh`, taking the half-winding as the boundary.
    ///
    /// A point exactly on the surface answers either way; a distance field does not care,
    /// because its value there is zero whichever sign it carries.
    pub fn is_inside(&self, mesh: &Mesh, point: Vec3) -> bool {
        self.at(mesh, point) > 0.5
    }
}

/// Builds the subtree covering `items`, which start at `offset` in the final face order,
/// and returns the index of its root node.
///
/// `items` is reordered in place, so a leaf is a contiguous stretch of it. The split
/// matches `bvh.rs`: a median on the widest axis of the centroids.
fn split(nodes: &mut Vec<Node>, items: &mut [Item], offset: usize, depth: usize) -> usize {
    let index = nodes.len();
    nodes.push(summarise(items, offset));

    if items.len() <= LEAF_FACES || depth == MAX_DEPTH {
        return index;
    }

    let axis = widest_axis(items);
    let (low, high) = items.iter().fold(
        (Scalar::INFINITY, Scalar::NEG_INFINITY),
        |(low, high), item| (low.min(item.centroid[axis]), high.max(item.centroid[axis])),
    );
    // Every face sitting on the same spot cannot be told apart by any split.
    if high - low <= 0.0 {
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

/// Collapses `items` into the node that stands for them: the area-weighted centre of their
/// centroids, a radius covering every vertex, and the sum of their area-weighted normals.
fn summarise(items: &[Item], offset: usize) -> Node {
    let area: Scalar = items.iter().map(|item| item.area).sum();
    let center = if area > 0.0 {
        items
            .iter()
            .map(|item| item.centroid * item.area)
            .fold(Vec3::ZERO, |sum, weighted| sum + weighted)
            / area
    } else {
        // Every face here has collapsed to a line or a point, and weighting by zero area
        // would divide by zero. They contribute no dipole either way.
        items
            .iter()
            .map(|item| item.centroid)
            .fold(Vec3::ZERO, |sum, centroid| sum + centroid)
            / items.len() as Scalar
    };

    Node {
        center,
        radius: items
            .iter()
            .map(|item| (item.centroid - center).length() + item.radius)
            .fold(0.0, Scalar::max),
        dipole: items
            .iter()
            .map(|item| item.dipole)
            .fold(Vec3::ZERO, |sum, dipole| sum + dipole),
        at: offset as u32,
        count: items.len() as u32,
    }
}

fn widest_axis(items: &[Item]) -> usize {
    let mut mins = Vec3::splat(Scalar::INFINITY);
    let mut maxs = Vec3::splat(Scalar::NEG_INFINITY);
    for item in items {
        mins = mins.min(item.centroid);
        maxs = maxs.max(item.centroid);
    }

    let extent = maxs - mins;
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

    /// A cube from the origin to `(size, size, size)`, wound outward.
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
            ],
        )
    }

    #[test]
    fn a_point_inside_a_closed_cube_winds_once() {
        let mesh = cube(10.0);
        let inside = winding_number(&mesh, Vec3::splat(5.0));
        assert!(
            (inside - 1.0).abs() < 1e-4,
            "a closed outward-wound surface subtends 4pi, so the winding is 1, got {inside}"
        );
    }

    #[test]
    fn a_point_outside_a_closed_cube_winds_not_at_all() {
        let outside = winding_number(&cube(10.0), Vec3::new(-4.0, 5.0, 5.0));
        assert!(outside.abs() < 1e-4, "got {outside}");
    }

    #[test]
    fn inverted_winding_flips_the_sign() {
        let mut mesh = cube(10.0);
        for face in &mut mesh.faces {
            face.swap(1, 2);
        }
        let inside = winding_number(&mesh, Vec3::splat(5.0));
        assert!((inside + 1.0).abs() < 1e-4, "got {inside}");
    }

    #[test]
    fn the_hierarchy_agrees_with_every_face() {
        let mesh = cube(10.0);
        let winding = Winding::build(&mesh);
        for point in [
            Vec3::splat(5.0),
            Vec3::new(0.1, 0.1, 0.1),
            Vec3::new(9.9, 5.0, 5.0),
            Vec3::new(-30.0, 40.0, 12.0),
            Vec3::new(5.0, 5.0, 10.5),
        ] {
            let exact = winding_number(&mesh, point);
            let approximate = winding.at(&mesh, point);
            assert!(
                (exact - approximate).abs() < 1e-3,
                "at {point}: every face gives {exact}, the hierarchy {approximate}"
            );
        }
    }

    #[test]
    fn a_cube_missing_a_face_still_reads_as_inside() {
        let mut mesh = cube(10.0);
        mesh.faces.truncate(10);
        let winding = Winding::build(&mesh);
        assert!(winding.is_inside(&mesh, Vec3::new(5.0, 5.0, 2.0)));
    }

    #[test]
    fn an_empty_mesh_winds_not_at_all() {
        let mesh = Mesh::default();
        let winding = Winding::build(&mesh);
        assert!(winding.is_empty());
        assert!(winding.at(&mesh, Vec3::ZERO).abs() < 1e-9);
    }
}
