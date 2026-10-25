use crate::{Aabb, Bvh, Mat3, Mat4, Mesh, Scalar, Transform, Triangle, Vec3};

/// Below this the ray runs parallel to the triangle's plane and there is no single
/// intersection. The determinant scales with the triangle's area in square millimetres,
/// so this rejects triangles thinner than about a nanometre across.
const PARALLEL_EPSILON: Scalar = 1e-9;

/// A half-line in some space, with a unit direction so that `t` is a distance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

impl Ray {
    /// Normalises `direction`, so every `t` this module returns is a length in the ray's
    /// own space. A zero direction gives a ray that hits nothing.
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Self {
            origin,
            direction: direction.normalize_or_zero(),
        }
    }

    pub fn at(&self, t: Scalar) -> Vec3 {
        self.origin + self.direction * t
    }
}

/// Where a ray met a mesh: the distance along the ray and which face it hit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayHit {
    pub t: Scalar,
    pub face: usize,
}

/// Distance at which `ray` enters `bounds`, zero if inside; `None` on a miss or behind the origin.
/// Slab method, with axes the ray does not travel along apart: `0 * inf` NaN is not SIMD-safe.
pub fn ray_aabb(ray: &Ray, bounds: &Aabb) -> Option<Scalar> {
    let mut entry: Scalar = 0.0;
    let mut exit = Scalar::INFINITY;

    for axis in 0..3 {
        let direction = ray.direction[axis];
        let (near, far) = (bounds.mins[axis], bounds.maxs[axis]);

        if direction == 0.0 {
            // Parallel to this pair of faces: either always between them, or never.
            if ray.origin[axis] < near || ray.origin[axis] > far {
                return None;
            }
            continue;
        }

        let inverse = 1.0 / direction;
        let first = (near - ray.origin[axis]) * inverse;
        let second = (far - ray.origin[axis]) * inverse;

        entry = entry.max(first.min(second));
        exit = exit.min(first.max(second));
        if exit < entry {
            return None;
        }
    }

    Some(entry)
}

/// Distance at which `ray` crosses `triangle`, by Möller-Trumbore; `None` on a miss, parallel or
/// backwards. Both faces count as a hit: picking must work on a model whose winding is still wrong.
pub fn ray_triangle(ray: &Ray, triangle: &Triangle) -> Option<Scalar> {
    let edge1 = triangle.b - triangle.a;
    let edge2 = triangle.c - triangle.a;

    let across = ray.direction.cross(edge2);
    let determinant = edge1.dot(across);
    if determinant.abs() < PARALLEL_EPSILON {
        return None;
    }
    let inverse = 1.0 / determinant;

    let to_origin = ray.origin - triangle.a;
    let u = to_origin.dot(across) * inverse;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }

    let along = to_origin.cross(edge1);
    let v = ray.direction.dot(along) * inverse;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }

    let t = edge2.dot(along) * inverse;
    (t >= 0.0).then_some(t)
}

/// Nearest face of `mesh` that `ray` hits, in the mesh's own space, by testing every face.
///
/// Linear in the face count. For a mesh of print size, and for anything cast more than
/// once, build a [`crate::Bvh`] instead; this is what it is measured and tested against.
pub fn raycast(mesh: &Mesh, ray: &Ray) -> Option<RayHit> {
    ray_aabb(ray, &mesh.aabb()?)?;

    (0..mesh.faces.len())
        .filter_map(|face| {
            let triangle = mesh.triangle(face)?;
            Some(RayHit {
                t: ray_triangle(ray, &triangle)?,
                face,
            })
        })
        .min_by(|a, b| a.t.total_cmp(&b.t))
}

/// Where a ray met a placed mesh, reported in the space the ray was given in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacedHit {
    pub point: Vec3,
    /// Unit normal of the face that was hit, as the placement leaves it. Zero for a
    /// degenerate face, which has no normal to report.
    pub normal: Vec3,
    /// Distance from the ray's origin along its direction, in the ray's own space.
    pub distance: Scalar,
    /// Index of the face that was hit, into the mesh's own faces.
    pub face: usize,
}

/// Nearest face of `mesh` placed by `transform` that `ray` hits, in the space the ray was given in.
/// The ray moves into mesh space, not the mesh into ray space; `bvh` is the hierarchy of `mesh`.
pub fn raycast_placed(
    mesh: &Mesh,
    bvh: &Bvh,
    transform: Transform,
    ray: &Ray,
) -> Option<PlacedHit> {
    let matrix = transform.to_matrix();
    let inverse = transform.inverse_matrix()?;
    raycast_placed_within(mesh, bvh, matrix, inverse, ray, Scalar::INFINITY)
}

/// [`raycast_placed`] with the placement given as its non-singular matrix and that
/// matrix's inverse, looking no further than `max_distance` along `ray`.
pub fn raycast_placed_within(
    mesh: &Mesh,
    bvh: &Bvh,
    matrix: Mat4,
    inverse: Mat4,
    ray: &Ray,
    max_distance: Scalar,
) -> Option<PlacedHit> {
    let along = inverse.transform_vector3(ray.direction);
    let local = Ray::new(inverse.transform_point3(ray.origin), along);

    // A unit step of the ray is `along` long in the mesh's own space.
    let hit = bvh.raycast_within(&local, max_distance * along.length())?;
    let point = matrix.transform_point3(local.at(hit.t));

    // A normal does not survive a non-uniform scale under the model matrix, but it does
    // under the inverse transpose of that matrix's upper 3x3.
    let normal = mesh
        .triangle(hit.face)
        .map_or(Vec3::ZERO, |triangle| {
            Mat3::from_mat4(inverse).transpose() * triangle.normal_unnormalized()
        })
        .normalize_or_zero();

    Some(PlacedHit {
        point,
        normal,
        distance: (point - ray.origin).dot(ray.direction),
        face: hit.face,
    })
}

#[cfg(test)]
mod tests;
