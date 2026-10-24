use super::*;
use crate::raycast;

/// A field of separate triangles, more than one thread builds alone.
fn scattered(count: u32) -> Mesh {
    let mut mesh = Mesh::default();
    for at in 0..count {
        let corner = Vec3::new(
            (at % 211) as Scalar,
            (at / 211 % 97) as Scalar,
            (at % 13) as Scalar,
        );
        let base = mesh.vertices.len() as u32;
        mesh.vertices
            .extend([corner, corner + Vec3::X * 0.5, corner + Vec3::Y * 0.5]);
        mesh.faces.push([base, base + 1, base + 2]);
    }
    mesh
}

#[test]
fn a_hierarchy_built_on_many_threads_is_the_one_one_thread_builds() {
    let mesh = scattered(4 * PARALLEL_FACES as u32 + 77);
    let built = Bvh::build(&mesh);

    let mut items: Vec<Item> = (0..mesh.faces.len())
        .filter_map(|face| item(&mesh, face))
        .collect();
    let mut nodes = Vec::new();
    split(&mut nodes, &mut items, 0, 0);

    assert_eq!(built.nodes, nodes);
    assert_eq!(
        built.faces,
        items.iter().map(|item| item.face).collect::<Vec<_>>()
    );
}

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
        let found = bvh.closest(point).expect("the cube has faces");
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
    crate::cuboid(Vec3::ZERO, Vec3::splat(size))
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
    assert_eq!(bvh.raycast(&Ray::new(Vec3::ZERO, Vec3::Z)), None);
}

#[test]
fn a_ray_down_the_middle_hits_the_top_face() {
    let mesh = cube(10.0);
    let bvh = Bvh::build(&mesh);
    let ray = Ray::new(Vec3::new(5.0, 5.0, 20.0), -Vec3::Z);

    let hit = bvh.raycast(&ray).expect("the cube is under the ray");
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
    assert_eq!(bvh.raycast(&ray), None);
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
            bvh.raycast(&ray).map(|hit| hit.face),
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
            bvh.raycast(&ray),
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
    assert!(bvh.raycast(&ray).is_some());
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
        let found = bvh.closest(point).expect("the cube has faces");
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
        .closest(Vec3::new(2.0, 5.0, 5.0))
        .expect("the cube has faces");
    assert!(
        (found.distance - 2.0).abs() < 1e-5,
        "got {}",
        found.distance
    );
    assert!((found.point - Vec3::new(0.0, 5.0, 5.0)).length() < 1e-5);
}

#[test]
fn the_nearest_point_kept_skips_a_nearer_face_left_out() {
    let mesh = cube(10.0);
    let bvh = Bvh::build(&mesh);
    let point = Vec3::new(2.0, 5.0, 5.0);
    let wall = bvh.closest(point).expect("the cube has faces").face;
    let found = bvh
        .closest_where(point, |face| face != wall && face != (wall ^ 1))
        .expect("other faces are kept");
    assert!(
        (found.distance - 5.0).abs() < 1e-5,
        "with the wall 2 mm off left out, the next is 5 mm off; got {}",
        found.distance
    );
    assert!(bvh.closest_where(point, |_| false).is_none());
}

#[test]
fn an_empty_hierarchy_has_no_nearest_point() {
    let mesh = Mesh::default();
    assert!(Bvh::build(&mesh).closest(Vec3::ZERO).is_none());
}
