#[path = "shared/bodies.rs"]
mod bodies;

use core_geometry::{
    Bvh, DEFAULT_WELD_TOLERANCE, Mesh, Quat, Ray, Scalar, Transform, Triangle, Vec3, Winding,
    closest_point, diagnose, orient_outward, point_triangle, ray_aabb, ray_triangle, raycast,
    signed_volume, transform_mesh, weld, winding_number,
};
use proptest::prelude::*;

fn any_vertex() -> impl Strategy<Value = Vec3> {
    (-100.0f32..100.0, -100.0f32..100.0, -100.0f32..100.0).prop_map(|(x, y, z)| Vec3::new(x, y, z))
}

fn any_rotation() -> impl Strategy<Value = Quat> {
    (-1.0f32..1.0, -1.0f32..1.0, -1.0f32..1.0, -6.3f32..6.3).prop_filter_map(
        "axis must not be zero",
        |(x, y, z, angle)| {
            let axis = Vec3::new(x, y, z);
            (axis.length_squared() > 1e-4).then(|| Quat::from_axis_angle(axis.normalize(), angle))
        },
    )
}

fn rigid(rotation: Quat, translation: Vec3) -> Transform {
    Transform {
        translation,
        rotation,
        scale: Vec3::ONE,
    }
}

/// Inverse of a rigid transform, in the same translate-after-rotate form.
fn rigid_inverse(transform: Transform) -> Transform {
    let rotation = transform.rotation.inverse();
    rigid(rotation, rotation * -transform.translation)
}

proptest! {
    #[test]
    fn welding_never_grows_the_bounds(points in prop::collection::vec(any_vertex(), 1..64)) {
        let original = Mesh::new(points, Vec::new());
        let before = original.aabb().expect("non-empty");
        let after = weld(&original, DEFAULT_WELD_TOLERANCE).mesh.aabb().expect("non-empty");

        let slack = Vec3::splat(DEFAULT_WELD_TOLERANCE);
        prop_assert!(after.mins.cmpge(before.mins - slack).all(), "{} vs {}", after.mins, before.mins);
        prop_assert!(after.maxs.cmple(before.maxs + slack).all(), "{} vs {}", after.maxs, before.maxs);
    }

    #[test]
    fn welding_keeps_every_vertex_within_the_tolerance(
        points in prop::collection::vec(any_vertex(), 1..64),
    ) {
        let original = Mesh::new(points, Vec::new());
        let welded = weld(&original, DEFAULT_WELD_TOLERANCE).mesh;

        for vertex in &original.vertices {
            let nearest = welded
                .vertices
                .iter()
                .map(|kept| kept.distance(*vertex))
                .fold(Scalar::INFINITY, Scalar::min);
            prop_assert!(
                nearest <= DEFAULT_WELD_TOLERANCE,
                "vertex moved {nearest} mm, tolerance is {DEFAULT_WELD_TOLERANCE}"
            );
        }
    }

    #[test]
    fn rigid_transform_preserves_volume(
        rotation in any_rotation(),
        translation in any_vertex(),
    ) {
        let cube = bodies::cube(10.0);
        let moved = transform_mesh(&cube, rigid(rotation, translation));
        prop_assert!((signed_volume(&moved) - 1000.0).abs() < 1.0);
    }

    #[test]
    fn uniform_scale_cubes_the_volume(factor in 0.1f32..8.0) {
        let scaled = transform_mesh(
            &bodies::cube(1.0),
            Transform { scale: Vec3::splat(factor), ..Transform::default() },
        );
        let expected = factor.powi(3);
        prop_assert!((signed_volume(&scaled) - expected).abs() < expected * 1e-3);
    }

    #[test]
    fn rigid_transform_then_inverse_returns_the_original_points(
        rotation in any_rotation(),
        translation in any_vertex(),
    ) {
        let cube = bodies::cube(10.0);
        let transform = rigid(rotation, translation);
        let round_trip = transform_mesh(&transform_mesh(&cube, transform), rigid_inverse(transform));

        for (before, after) in cube.vertices.iter().zip(&round_trip.vertices) {
            prop_assert!(before.distance(*after) < 1e-3, "{before} became {after}");
        }
    }

    #[test]
    fn a_closed_shell_stays_closed_and_positive_after_repair(
        inverted in 0usize..12,
        rings in 3usize..12,
        segments in 3usize..12,
    ) {
        let mut mesh = bodies::uv_sphere(4.0, rings, segments);
        for face in mesh.faces.iter_mut().take(inverted) {
            face.swap(1, 2);
        }
        let report = orient_outward(&mut mesh);

        prop_assert!(report.orientable);
        prop_assert!(diagnose(&mesh).is_closed());
        prop_assert!(signed_volume(&mesh) > 0.0);
    }

    #[test]
    fn orienting_is_idempotent(inverted in 0usize..12) {
        let mut mesh = bodies::cube_with_inverted_faces(3.0, inverted);
        orient_outward(&mut mesh);
        let once = mesh.faces.clone();

        let second = orient_outward(&mut mesh);
        prop_assert!(second.unchanged());
        prop_assert_eq!(once, mesh.faces);
    }
}

proptest! {
    /// A reported hit is a point of the triangle, not merely a point of its plane.
    #[test]
    fn a_triangle_hit_lands_inside_the_triangle(
        a in any_vertex(),
        b in any_vertex(),
        c in any_vertex(),
        origin in any_vertex(),
        target in any_vertex(),
    ) {
        let triangle = Triangle::new(a, b, c);
        prop_assume!(triangle.area() > 1e-2);

        let ray = Ray::new(origin, target - origin);
        prop_assume!(ray.direction != Vec3::ZERO);

        if let Some(t) = ray_triangle(&ray, &triangle) {
            let point = ray.at(t);
            // Barycentric coordinates of the hit, by the ratio of the sub-triangle areas.
            let total = triangle.area();
            let alpha = Triangle::new(point, b, c).area() / total;
            let beta = Triangle::new(a, point, c).area() / total;
            let gamma = Triangle::new(a, b, point).area() / total;

            prop_assert!(
                (alpha + beta + gamma - 1.0).abs() < 1e-2,
                "the sub-triangles of a point inside sum to the whole, got {}",
                alpha + beta + gamma
            );
            prop_assert!(t >= 0.0, "a hit is never behind the ray's origin");
        }
    }

    /// Whatever the ray does, the mesh it enters is entered no sooner than its bounds.
    #[test]
    fn a_mesh_hit_is_never_nearer_than_its_bounding_box(
        origin in any_vertex(),
        target in any_vertex(),
    ) {
        let mesh = bodies::cube(1.0);
        let ray = Ray::new(origin, target - origin);
        prop_assume!(ray.direction != Vec3::ZERO);

        if let Some(hit) = raycast(&mesh, &ray) {
            let bounds = mesh.aabb().expect("the cube has vertices");
            let entry = ray_aabb(&ray, &bounds).expect("a face hit implies a box hit");
            prop_assert!(hit.t >= entry - 1e-3, "{} is nearer than the box at {}", hit.t, entry);
        }
    }
}

proptest! {
    /// Whatever the point, the hierarchy finds what testing every face finds.
    #[test]
    fn the_nearest_point_is_the_one_every_face_agrees_on(point in any_vertex()) {
        let mesh = bodies::octahedron(7.0);
        let bvh = Bvh::build(&mesh);

        let exact = closest_point(&mesh, point).expect("the octahedron has faces");
        let found = bvh.closest(&mesh, point).expect("the octahedron has faces");
        prop_assert!(
            (found.distance - exact.distance).abs() < 1e-3,
            "every face gives {}, the hierarchy {}",
            exact.distance,
            found.distance
        );
    }

    /// The point handed back lies on the face handed back, at the distance handed back.
    #[test]
    fn the_nearest_point_lies_on_the_face_it_names(point in any_vertex()) {
        let mesh = bodies::octahedron(7.0);
        let bvh = Bvh::build(&mesh);
        let found = bvh.closest(&mesh, point).expect("the octahedron has faces");

        let triangle = mesh.triangle(found.face).expect("the face was named by the mesh");
        let on_face = point_triangle(found.point, &triangle);
        prop_assert!((on_face - found.point).length() < 1e-3);
        prop_assert!(((found.point - point).length() - found.distance).abs() < 1e-3);
    }

    /// The dipole expansion only ever stands in for faces far enough away to be summarised,
    /// so its answer has to hold wherever the point is, to within the few percent of a
    /// winding that the stand-off buys; see `FAR_ENOUGH` in `winding.rs`.
    #[test]
    fn the_winding_hierarchy_agrees_with_every_face(point in any_vertex()) {
        let mesh = bodies::cube(10.0);
        let winding = Winding::build(&mesh);

        let exact = winding_number(&mesh, point);
        let approximate = winding.at(&mesh, point);
        prop_assert!(
            (exact - approximate).abs() < 0.05,
            "every face gives {exact}, the hierarchy {approximate}"
        );
    }
}
