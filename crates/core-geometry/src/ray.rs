use crate::{Aabb, Bvh, Mat3, Mesh, Scalar, Transform, Triangle, Vec3};

/// Below this the ray runs parallel to the triangle's plane and there is no single
/// intersection. The determinant scales with the triangle's area in square millimetres,
/// so this rejects triangles thinner than about a nanometre across.
const PARALLEL_EPSILON: Scalar = 1e-9;

/// A transform this close to singular has flattened its mesh into a plane or a point. It
/// cannot be inverted, and there is nothing left to hit either.
const SINGULAR: Scalar = 1e-12;

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

/// Distance at which `ray` enters `bounds`, or zero if it starts inside. `None` when it
/// misses or when the box is entirely behind the origin.
///
/// The slab method, with the axes the ray does not travel along handled separately. The
/// branchless form divides by zero on those axes and relies on `0 * inf` producing a NaN
/// that `min` then discards, which holds for scalar `f32` but not for every SIMD backend
/// `glam` compiles to.
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

/// Distance at which `ray` crosses `triangle`, by Möller-Trumbore. `None` when it misses,
/// runs in the triangle's plane, or would have to travel backwards to get there.
///
/// Both faces count as a hit. Picking has to work on a model whose winding is still wrong,
/// which is exactly the model the user wants to select and inspect.
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

/// Nearest face of `mesh` placed by `transform` that `ray` hits, with the hit point and
/// normal brought back out to the space the ray was given in.
///
/// `bvh` is the hierarchy of `mesh`, which is built once and kept beside it. The ray is
/// moved into the mesh's own space rather than the mesh into the ray's: the mesh is
/// millions of vertices and the ray is two, and the hierarchy only holds for the space it
/// was built in. Hits under different scales are therefore still comparable against each
/// other, which picking depends on.
pub fn raycast_placed(
    mesh: &Mesh,
    bvh: &Bvh,
    transform: Transform,
    ray: &Ray,
) -> Option<PlacedHit> {
    let matrix = transform.to_matrix();
    if matrix.determinant().abs() < SINGULAR {
        return None;
    }

    let inverse = matrix.inverse();
    let local = Ray::new(
        inverse.transform_point3(ray.origin),
        inverse.transform_vector3(ray.direction),
    );

    let hit = bvh.raycast(mesh, &local)?;
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
mod tests {
    use super::*;

    /// Axis-aligned cube spanning 0..1 on every axis, twelve triangles wound outwards.
    fn unit_cube() -> Mesh {
        let vertices = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, 1.0),
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(0.0, 1.0, 1.0),
        ];
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

    /// Casts against a mesh through the hierarchy every caller outside the tests holds.
    fn placed(mesh: &Mesh, transform: Transform, ray: &Ray) -> Option<PlacedHit> {
        raycast_placed(mesh, &Bvh::build(mesh), transform, ray)
    }

    fn right_triangle() -> Triangle {
        Triangle::new(Vec3::ZERO, Vec3::X, Vec3::Y)
    }

    #[test]
    fn a_ray_down_the_axis_hits_the_top_of_the_cube() {
        let ray = Ray::new(Vec3::new(0.5, 0.5, 4.0), -Vec3::Z);
        let hit = raycast(&unit_cube(), &ray).expect("the ray goes through the cube");
        // The eye is 4 mm up and the top face is at z = 1, so the first hit is at 3 mm.
        assert!(
            (hit.t - 3.0).abs() < 1e-5,
            "expected t = 4 - 1, got {}",
            hit.t
        );
    }

    #[test]
    fn the_near_face_wins_over_the_far_one() {
        let ray = Ray::new(Vec3::new(-4.0, 0.5, 0.5), Vec3::X);
        let hit = raycast(&unit_cube(), &ray).expect("the ray crosses the cube");
        assert!(
            (hit.t - 4.0).abs() < 1e-5,
            "the x = 0 face is nearer than x = 1"
        );
    }

    #[test]
    fn a_ray_pointing_away_misses() {
        let ray = Ray::new(Vec3::new(0.5, 0.5, 4.0), Vec3::Z);
        assert!(raycast(&unit_cube(), &ray).is_none());
    }

    #[test]
    fn a_ray_beside_the_cube_misses() {
        let ray = Ray::new(Vec3::new(5.0, 0.5, 0.5), -Vec3::Z);
        assert!(raycast(&unit_cube(), &ray).is_none());
    }

    #[test]
    fn a_ray_parallel_to_the_triangle_misses() {
        let ray = Ray::new(Vec3::new(-1.0, 0.25, 1.0), Vec3::X);
        assert!(ray_triangle(&ray, &right_triangle()).is_none());
    }

    #[test]
    fn a_ray_inside_the_triangles_plane_misses() {
        let ray = Ray::new(Vec3::new(-1.0, 0.25, 0.0), Vec3::X);
        assert!(ray_triangle(&ray, &right_triangle()).is_none());
    }

    #[test]
    fn the_corner_outside_the_hypotenuse_is_not_covered() {
        // (0.9, 0.9) is inside the unit square but outside x + y <= 1.
        let ray = Ray::new(Vec3::new(0.9, 0.9, 1.0), -Vec3::Z);
        assert!(ray_triangle(&ray, &right_triangle()).is_none());
        let inside = Ray::new(Vec3::new(0.2, 0.2, 1.0), -Vec3::Z);
        assert!(ray_triangle(&inside, &right_triangle()).is_some());
    }

    #[test]
    fn a_back_face_still_counts_as_a_hit() {
        let ray = Ray::new(Vec3::new(0.25, 0.25, -1.0), Vec3::Z);
        let t = ray_triangle(&ray, &right_triangle()).expect("the back face is hit");
        assert!((t - 1.0).abs() < 1e-5);
    }

    #[test]
    fn a_ray_starting_inside_the_box_enters_at_zero() {
        let bounds = Aabb::new(Vec3::ZERO, Vec3::ONE);
        let ray = Ray::new(Vec3::splat(0.5), Vec3::X);
        assert_eq!(ray_aabb(&ray, &bounds), Some(0.0));
    }

    #[test]
    fn a_ray_along_the_face_of_the_box_still_hits_it() {
        // Degenerate slab: the ray lies in the z = 0 plane of the box.
        let bounds = Aabb::new(Vec3::ZERO, Vec3::ONE);
        let ray = Ray::new(Vec3::new(-2.0, 0.5, 0.0), Vec3::X);
        assert_eq!(ray_aabb(&ray, &bounds), Some(2.0));
    }

    #[test]
    fn an_empty_mesh_is_never_hit() {
        let ray = Ray::new(Vec3::ZERO, Vec3::X);
        assert!(raycast(&Mesh::default(), &ray).is_none());
    }

    #[test]
    fn a_placed_hit_comes_back_in_the_rays_own_space() {
        let transform = Transform {
            translation: Vec3::new(10.0, 0.0, 0.0),
            scale: Vec3::splat(2.0),
            ..Transform::default()
        };
        // The cube now spans 10..12 in x and 0..2 in y and z, so its top is at z = 2.
        let ray = Ray::new(Vec3::new(11.0, 1.0, 5.0), -Vec3::Z);
        let hit = placed(&unit_cube(), transform, &ray).expect("the ray goes through");

        assert!(hit.point.abs_diff_eq(Vec3::new(11.0, 1.0, 2.0), 1e-5));
        assert!((hit.distance - 3.0).abs() < 1e-5, "5 mm up, top at z = 2");
        assert!(
            hit.normal.abs_diff_eq(Vec3::Z, 1e-5),
            "the top face faces up"
        );
    }

    #[test]
    fn a_non_uniform_scale_leaves_the_normal_perpendicular() {
        // A 45 degree slope whose normal is (-1, 0, 1)/sqrt(2) before scaling.
        let mesh = Mesh::new(
            vec![
                Vec3::new(0.0, -1.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(1.0, 0.0, 1.0),
            ],
            vec![[0, 1, 2]],
        );
        let transform = Transform {
            scale: Vec3::new(1.0, 1.0, 4.0),
            ..Transform::default()
        };

        let ray = Ray::new(Vec3::new(0.5, 0.0, 10.0), -Vec3::Z);
        let hit = placed(&mesh, transform, &ray).expect("the ray meets the slope");

        // Stretching z by four steepens the face, so its normal must tilt the other way.
        let along = Vec3::new(1.0, 0.0, 4.0).normalize();
        assert!(
            hit.normal.dot(along).abs() < 1e-5,
            "expected a normal perpendicular to the stretched slope, got {}",
            hit.normal
        );
    }

    #[test]
    fn a_flattened_placement_is_never_hit() {
        let flat = Transform {
            scale: Vec3::new(1.0, 1.0, 0.0),
            ..Transform::default()
        };
        let ray = Ray::new(Vec3::new(0.5, 0.5, 4.0), -Vec3::Z);
        assert!(placed(&unit_cube(), flat, &ray).is_none());
    }

    #[test]
    fn a_degenerate_face_is_hit_without_a_normal() {
        // A sliver with three collinear vertices has area zero and no normal, but
        // Moller-Trumbore still reports a crossing for a ray through its interior.
        let mesh = Mesh::new(
            vec![Vec3::ZERO, Vec3::X, Vec3::new(2.0, 0.0, 0.0)],
            vec![[0, 1, 2]],
        );
        let ray = Ray::new(Vec3::new(0.5, 0.0, 1.0), -Vec3::Z);
        assert!(placed(&mesh, Transform::default(), &ray).is_none());
    }

    #[test]
    fn a_zero_direction_hits_nothing() {
        let ray = Ray::new(Vec3::splat(0.5), Vec3::ZERO);
        assert_eq!(ray.direction, Vec3::ZERO);
        assert!(ray_triangle(&ray, &right_triangle()).is_none());
    }
}
