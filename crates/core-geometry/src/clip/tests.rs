use super::*;
use crate::{ball, diagnose, signed_volume};

/// Axis-aligned box, twelve triangles, spanning `min`..`max`.
fn cuboid(min: Vec3, max: Vec3) -> Mesh {
    crate::cuboid(min, max)
}

fn at_height(height_mm: Scalar) -> Plane {
    Plane::at(Vec3::Z, height_mm).expect("Z is a direction")
}

#[test]
fn a_cube_cut_in_half_gives_two_closed_halves() {
    let cube = cuboid(Vec3::ZERO, Vec3::splat(10.0));
    let halves = cut(&cube, at_height(4.0));

    assert_eq!(halves.open_loops, 0, "a closed cube cuts into closed loops");
    for (name, half, expected) in [
        ("below", &halves.below, 400.0),
        ("above", &halves.above, 600.0),
    ] {
        let diagnostics = diagnose(half);
        assert!(
            diagnostics.is_closed(),
            "the {name} half is closed, got {diagnostics:?}"
        );
        let volume = signed_volume(half);
        assert!(
            (volume - expected).abs() < 1e-2,
            "the {name} half holds {expected} mm3, got {volume}"
        );
    }
}

#[test]
fn the_two_halves_hold_what_the_whole_held() {
    let cube = cuboid(Vec3::ZERO, Vec3::splat(10.0));
    let halves = cut(&cube, at_height(2.5));
    let total = signed_volume(&halves.below) + signed_volume(&halves.above);
    assert!((total - signed_volume(&cube)).abs() < 1e-2, "got {total}");
}

#[test]
fn a_cut_through_a_cavity_caps_the_wall_and_not_the_hole() {
    // A hollow box: an outer shell with an inner one wound the other way.
    let mut hollow = cuboid(Vec3::ZERO, Vec3::splat(10.0));
    let inner = cuboid(Vec3::splat(2.0), Vec3::splat(8.0));
    let offset = hollow.vertices.len() as u32;
    hollow.vertices.extend(inner.vertices.iter().copied());
    hollow.faces.extend(
        inner
            .faces
            .iter()
            .map(|[a, b, c]| [a + offset, c + offset, b + offset]),
    );

    let halves = cut(&hollow, at_height(5.0));
    assert_eq!(halves.open_loops, 0);

    // 10x10x5 of box less 6x6x3 of cavity: the cap is a frame, not a disc.
    let volume = signed_volume(&halves.below);
    assert!(
        (volume - (500.0 - 108.0)).abs() < 1e-1,
        "expected the wall only, got {volume}"
    );
}

/// Both halves closed, wound outward and holding `below` and `above` mm³.
fn halves_hold(halves: &Cut, below: Scalar, above: Scalar) {
    assert_eq!(halves.open_loops, 0, "every loop of the section closes");
    for (name, half, expected) in [
        ("below", &halves.below, below),
        ("above", &halves.above, above),
    ] {
        let report = diagnose(half);
        assert!(
            report.is_sound() && report.unbalanced_edges == 0 && report.non_manifold_edges == 0,
            "the {name} half is closed, each edge walked once each way; got {report:?}"
        );
        let volume = signed_volume(half);
        assert!(
            (volume - expected).abs() < 1e-2,
            "the {name} half holds {expected} mm³, got {volume}"
        );
    }
}

#[test]
fn rings_nested_two_deep_are_each_capped_once() {
    // A 40 mm box round a 30 mm cavity round a 20 mm box round a 10 mm cavity: the
    // innermost hole lies inside two outer rings and belongs to the nearer.
    let mut nested = Mesh::default();
    for (low, side, inward) in [
        (0.0, 40.0, false),
        (5.0, 30.0, true),
        (10.0, 20.0, false),
        (15.0, 10.0, true),
    ] {
        let mut shell = cuboid(Vec3::splat(low), Vec3::splat(low + side));
        if inward {
            for face in &mut shell.faces {
                face.swap(1, 2);
            }
        }
        nested.append(&shell);
    }
    // Each half holds 40² × 20 − 30² × 15 + 20² × 10 − 10² × 5 mm³ of the four.
    halves_hold(&cut(&nested, at_height(20.0)), 22_000.0, 22_000.0);
}

#[test]
fn a_plane_along_a_step_of_the_model_closes_both_halves() {
    // An L 4 mm tall whose inner side at x = 5 lies in the plane: wall facing out of
    // the half below it, and with the plane turned round, out of the half above.
    let outline = [
        Vec2::new(0.0, 0.0),
        Vec2::new(10.0, 0.0),
        Vec2::new(10.0, 5.0),
        Vec2::new(5.0, 5.0),
        Vec2::new(5.0, 10.0),
        Vec2::new(0.0, 10.0),
    ];
    let step = crate::prism(&outline, 0.0, 4.0);
    let along_x = Plane::at(Vec3::X, 5.0).expect("X is a direction");
    halves_hold(&cut(&step, along_x), 200.0, 100.0);
    let against_x = Plane::at(-Vec3::X, -5.0).expect("X is a direction");
    halves_hold(&cut(&step, against_x), 100.0, 200.0);
}

#[test]
fn a_dense_ball_cut_aslant_near_its_pole_closes_both_halves() {
    // Near a pole the section crosses rings of tiny faces, and its fill has slivers
    // too thin for the sign of their own area to say which way they face.
    let ball = ball(Vec3::ZERO, 40.0, 400, 400);
    for normal in [
        Vec3::new(0.3, 0.5, 1.0),
        Vec3::new(-0.7, 0.2, 0.4),
        Vec3::new(0.1, 0.0, 1.0),
    ] {
        for height in [36.5, 38.2, 39.3] {
            let plane = Plane::at(normal, height).expect("a direction");
            let halves = cut(&ball, plane);
            for half in [&halves.below, &halves.above] {
                let report = diagnose(half);
                assert!(
                    report.is_sound() && report.unbalanced_edges == 0,
                    "cut {height} mm along {normal}: each edge walked once each way; got {report:?}"
                );
            }
        }
    }
}

#[test]
fn a_plane_that_misses_the_model_leaves_it_whole() {
    let cube = cuboid(Vec3::ZERO, Vec3::splat(10.0));
    let halves = cut(&cube, at_height(20.0));

    assert!(halves.above.is_empty(), "nothing is above the plane");
    assert_eq!(halves.below.faces.len(), cube.faces.len());
    assert!((signed_volume(&halves.below) - 1000.0).abs() < 1e-3);
}

#[test]
fn a_plane_along_a_face_cuts_nothing_off() {
    let cube = cuboid(Vec3::ZERO, Vec3::splat(10.0));
    let halves = cut(&cube, at_height(10.0));
    assert!(halves.above.is_empty(), "the top face is not a half");
    assert!((signed_volume(&halves.below) - 1000.0).abs() < 1e-3);
}

#[test]
fn a_cut_across_the_diagonal_still_closes() {
    let cube = cuboid(Vec3::ZERO, Vec3::splat(10.0));
    let plane = Plane {
        point: Vec3::splat(5.0),
        normal: Vec3::new(1.0, 1.0, 1.0).normalize(),
    };
    let halves = cut(&cube, plane);

    assert_eq!(halves.open_loops, 0);
    assert!(
        diagnose(&halves.below).is_closed(),
        "the low half is closed"
    );
    assert!(
        diagnose(&halves.above).is_closed(),
        "the high half is closed"
    );
    let total = signed_volume(&halves.below) + signed_volume(&halves.above);
    assert!((total - 1000.0).abs() < 1e-1, "got {total}");
}

/// Regular octahedron of circumradius `r`, with four of its vertices on z = 0.
fn octahedron(r: Scalar) -> Mesh {
    let vertices = vec![
        Vec3::new(r, 0.0, 0.0),
        Vec3::new(0.0, r, 0.0),
        Vec3::new(-r, 0.0, 0.0),
        Vec3::new(0.0, -r, 0.0),
        Vec3::new(0.0, 0.0, r),
        Vec3::new(0.0, 0.0, -r),
    ];
    let faces = vec![
        [0, 1, 4],
        [1, 2, 4],
        [2, 3, 4],
        [3, 0, 4],
        [1, 0, 5],
        [2, 1, 5],
        [3, 2, 5],
        [0, 3, 5],
    ];
    Mesh::new(vertices, faces)
}

#[test]
fn a_plane_through_a_ring_of_vertices_still_caps_both_halves() {
    // Nothing straddles: every face has an edge on the plane and its third corner
    // off it, which is what a lathed model gives at its equator.
    let halves = cut(&octahedron(10.0), at_height(0.0));

    assert_eq!(halves.open_loops, 0);
    for (name, half) in [("below", &halves.below), ("above", &halves.above)] {
        let diagnostics = diagnose(half);
        assert!(
            diagnostics.is_closed(),
            "the {name} half is closed, got {diagnostics:?}"
        );
        // Each half is a square pyramid of base diagonal 2r and height r:
        // (2r)^2 / 2 * r / 3 = 2 r^3 / 3.
        let volume = signed_volume(half);
        assert!(
            (volume - 2000.0 / 3.0).abs() < 1e-1,
            "the {name} half is half the octahedron, got {volume}"
        );
    }
}

#[test]
fn a_vertex_on_the_plane_is_one_vertex_in_the_half_that_keeps_it() {
    // x + 10z = 100 holds two of the cube's corners exactly and cuts four faces.
    let cube = cuboid(Vec3::ZERO, Vec3::splat(10.0));
    let plane = Plane {
        point: Vec3::new(0.0, 0.0, 10.0),
        normal: Vec3::new(1.0, 0.0, 10.0).normalize(),
    };
    let halves = cut(&cube, plane);

    assert_eq!(halves.open_loops, 0);
    for (name, half, expected) in [
        ("below", &halves.below, 950.0),
        ("above", &halves.above, 50.0),
    ] {
        let diagnostics = diagnose(half);
        assert!(
            diagnostics.is_closed(),
            "the {name} half is closed, got {diagnostics:?}"
        );
        // The wedge above is x/10 high over the 10 x 10 top: 50 mm3 of the 1000.
        let volume = signed_volume(half);
        assert!(
            (volume - expected).abs() < 1e-1,
            "the {name} half holds {expected} mm3, got {volume}"
        );
    }
}

#[test]
fn a_plane_with_no_direction_is_not_a_plane() {
    assert!(Plane::at(Vec3::ZERO, 1.0).is_none());
}
