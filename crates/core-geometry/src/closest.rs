use crate::{Aabb, Mesh, Scalar, Triangle, Vec3};

/// Where a mesh comes nearest to a query point.
///
/// The distance is unsigned: which side of the surface the point is on is a separate
/// question, answered by [`crate::Winding`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClosestPoint {
    /// The point on the surface, in the mesh's own space.
    pub point: Vec3,
    /// Distance in millimetres from the query point to `point`.
    pub distance: Scalar,
    pub face: usize,
}

/// Squared distance from `point` to the nearest point of `bounds`, zero when inside.
///
/// Squared so that a traversal can compare it against the best distance so far without a
/// square root per node.
pub fn point_aabb_squared(point: Vec3, bounds: &Aabb) -> Scalar {
    let outside = (bounds.mins - point)
        .max(point - bounds.maxs)
        .max(Vec3::ZERO);
    outside.length_squared()
}

/// Nearest point of `triangle` to `point`, by the Voronoi region test of Ericson,
/// *Real-Time Collision Detection* §5.1.5; see `docs/design/distance-queries.md`.
///
/// The seven regions — three vertices, three edges, the face — are tested in an order that
/// lets each one reuse the dot products the last one computed.
pub fn point_triangle(point: Vec3, triangle: &Triangle) -> Vec3 {
    let (a, b, c) = (triangle.a, triangle.b, triangle.c);
    let ab = b - a;
    let ac = c - a;

    let to_a = point - a;
    let d1 = ab.dot(to_a);
    let d2 = ac.dot(to_a);
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }

    let to_b = point - b;
    let d3 = ab.dot(to_b);
    let d4 = ac.dot(to_b);
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }

    let to_c = point - c;
    let d5 = ab.dot(to_c);
    let d6 = ac.dot(to_c);
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }

    let along_ab = d1 * d4 - d3 * d2;
    if along_ab <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }

    let along_ac = d5 * d2 - d1 * d6;
    if along_ac <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }

    let along_bc = d3 * d6 - d5 * d4;
    if along_bc <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }

    // A face with no area has no interior to project onto, and the three region tests
    // above can all miss on one. `Bvh::build` keeps such faces, so this has to hold.
    let total = along_ab + along_ac + along_bc;
    if total <= 0.0 {
        return [a, b, c]
            .into_iter()
            .min_by(|p, q| {
                (*p - point)
                    .length_squared()
                    .total_cmp(&(*q - point).length_squared())
            })
            .unwrap_or(a);
    }

    let inverse = 1.0 / total;
    a + ab * (along_ac * inverse) + ac * (along_ab * inverse)
}

/// Nearest point of `mesh` to `point`, in the mesh's own space, by testing every face.
///
/// Linear in the face count. A narrow-band distance field asks this of every voxel near
/// the surface, so build a [`crate::Bvh`] instead; this is what it is measured and tested
/// against.
pub fn closest_point(mesh: &Mesh, point: Vec3) -> Option<ClosestPoint> {
    (0..mesh.faces.len())
        .filter_map(|face| {
            let triangle = mesh.triangle(face)?;
            let on_face = point_triangle(point, &triangle);
            Some((face, on_face, (on_face - point).length_squared()))
        })
        .min_by(|a, b| a.2.total_cmp(&b.2))
        .map(|(face, on_face, squared)| ClosestPoint {
            point: on_face,
            distance: squared.sqrt(),
            face,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn right_triangle() -> Triangle {
        Triangle::new(Vec3::ZERO, Vec3::X * 2.0, Vec3::Y * 2.0)
    }

    #[test]
    fn a_point_over_the_face_projects_onto_it() {
        let found = point_triangle(Vec3::new(0.5, 0.5, 3.0), &right_triangle());
        assert!((found - Vec3::new(0.5, 0.5, 0.0)).length() < 1e-6);
    }

    #[test]
    fn a_point_past_a_vertex_snaps_to_it() {
        let found = point_triangle(Vec3::new(-4.0, -4.0, 1.0), &right_triangle());
        assert_eq!(found, Vec3::ZERO);
    }

    #[test]
    fn a_point_beside_an_edge_projects_onto_that_edge() {
        // Outside the hypotenuse from (2,0,0) to (0,2,0), level with its midpoint.
        let found = point_triangle(Vec3::new(3.0, 3.0, 0.0), &right_triangle());
        assert!((found - Vec3::new(1.0, 1.0, 0.0)).length() < 1e-6);
    }

    #[test]
    fn a_degenerate_face_answers_with_its_nearest_vertex() {
        let collapsed = Triangle::new(Vec3::ZERO, Vec3::X, Vec3::X);
        let found = point_triangle(Vec3::new(4.0, 0.0, 0.0), &collapsed);
        assert_eq!(found, Vec3::X);
    }

    #[test]
    fn distance_to_a_box_is_zero_inside_it() {
        let bounds = Aabb::new(Vec3::ZERO, Vec3::splat(2.0));
        assert!(point_aabb_squared(Vec3::ONE, &bounds) < 1e-9);
        // Three millimetres past the far corner along X alone.
        assert!((point_aabb_squared(Vec3::new(5.0, 1.0, 1.0), &bounds) - 9.0).abs() < 1e-6);
    }

    #[test]
    fn an_empty_mesh_has_no_closest_point() {
        assert!(closest_point(&Mesh::default(), Vec3::ZERO).is_none());
    }
}
