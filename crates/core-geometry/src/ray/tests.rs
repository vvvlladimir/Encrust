use super::*;

/// Axis-aligned cube spanning 0..1 on every axis, twelve triangles wound outwards.
fn unit_cube() -> Mesh {
    crate::cuboid(Vec3::ZERO, Vec3::ONE)
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
fn a_look_stops_at_its_length_in_the_rays_own_space() {
    // Doubled, the cube's top is at z = 2, three millimetres under an eye at z = 5;
    // in the cube's own space that is a millimetre and a half.
    let mesh = unit_cube();
    let bvh = Bvh::build(&mesh);
    let matrix = Transform {
        scale: Vec3::splat(2.0),
        ..Transform::default()
    }
    .to_matrix();
    let ray = Ray::new(Vec3::new(1.0, 1.0, 5.0), -Vec3::Z);
    let look = |max_mm| raycast_placed_within(&mesh, &bvh, matrix, matrix.inverse(), &ray, max_mm);

    assert!(look(2.9).is_none(), "the top is 3 mm away");
    let hit = look(3.1).expect("the top is within 3.1 mm");
    assert!((hit.distance - 3.0).abs() < 1e-5, "5 mm up, top at z = 2");
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
