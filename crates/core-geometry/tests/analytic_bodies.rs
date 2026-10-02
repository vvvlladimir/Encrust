#[path = "shared/bodies.rs"]
mod bodies;

use std::f32::consts::PI;

use core_geometry::{Bvh, Vec3, Winding, diagnose, signed_volume};

#[test]
fn cube_topology_and_volume_match_the_closed_form() {
    let mesh = bodies::cube(10.0);
    let diagnostics = diagnose(&mesh);

    assert_eq!(diagnostics.vertices, 8);
    assert_eq!(diagnostics.faces, 12);
    assert_eq!(diagnostics.boundary_edges, 0);
    assert_eq!(diagnostics.non_manifold_edges, 0);
    assert_eq!(diagnostics.shells, 1);
    assert_eq!(
        diagnostics.euler_characteristic, 2,
        "a closed sphere-like shell gives 2"
    );
    assert!(diagnostics.is_sound());
    assert!(
        (signed_volume(&mesh) - 1000.0).abs() < 1e-3,
        "10mm cube encloses 1000 mm^3"
    );
}

#[test]
fn tetrahedron_encloses_one_sixth() {
    let mesh = bodies::tetrahedron();
    assert!(diagnose(&mesh).is_closed());
    assert!((signed_volume(&mesh) - 1.0 / 6.0).abs() < 1e-6);
}

#[test]
fn octahedron_encloses_four_thirds_of_the_cubed_radius() {
    let mesh = bodies::octahedron(3.0);
    let diagnostics = diagnose(&mesh);

    assert_eq!(diagnostics.faces, 8);
    assert_eq!(diagnostics.euler_characteristic, 2);
    assert!(diagnostics.is_closed());
    assert!((signed_volume(&mesh) - 4.0 / 3.0 * 27.0).abs() < 1e-4);
}

#[test]
fn sphere_volume_approaches_the_analytic_value_from_below() {
    let radius: f32 = 5.0;
    let exact = 4.0 / 3.0 * PI * radius.powi(3);

    let coarse = signed_volume(&bodies::uv_sphere(radius, 16, 16));
    let fine = signed_volume(&bodies::uv_sphere(radius, 64, 64));

    assert!(
        coarse < exact,
        "an inscribed polyhedron cannot enclose more than the sphere"
    );
    assert!(fine < exact);
    assert!(
        fine > coarse,
        "refining the mesh must bring the volume closer"
    );
    assert!(
        fine > exact * 0.995,
        "64x64 should be within 0.5%, got {fine} against {exact}"
    );
}

#[test]
fn sphere_stays_closed_at_every_resolution() {
    for (rings, segments) in [(2, 3), (3, 8), (32, 17)] {
        let diagnostics = diagnose(&bodies::uv_sphere(1.0, rings, segments));
        assert!(diagnostics.is_closed(), "{rings}x{segments} sphere is open");
        assert_eq!(
            diagnostics.euler_characteristic, 2,
            "{rings}x{segments} sphere"
        );
        assert_eq!(diagnostics.degenerate_faces, 0, "{rings}x{segments} sphere");
    }
}

#[test]
fn open_box_reports_its_boundary_loop() {
    let diagnostics = diagnose(&bodies::open_box(4.0));

    assert_eq!(
        diagnostics.boundary_edges, 4,
        "the missing lid leaves a square hole"
    );
    assert_eq!(diagnostics.non_manifold_edges, 0);
    assert_eq!(diagnostics.euler_characteristic, 1, "a disc gives 1");
    assert!(!diagnostics.is_closed());
}

#[test]
fn distance_to_a_sphere_is_the_offset_from_its_radius() {
    let radius: f32 = 20.0;
    let mesh = bodies::uv_sphere(radius, 96, 96);
    let bvh = Bvh::build(&mesh);
    // The mesh is inscribed, so a facet sags below the sphere by the chord error of half
    // a ring in latitude and half a segment in longitude together: 13 micrometres at this
    // radius. Everything here is measured against that.
    let sag = radius * (1.0 - (PI / 192.0).cos() * (PI / 96.0).cos());

    for point in [
        Vec3::new(30.0, 0.0, 0.0),
        Vec3::new(0.0, 0.0, 25.0),
        Vec3::new(12.0, 12.0, 12.0),
        Vec3::ZERO,
        Vec3::new(3.0, -4.0, 5.0),
    ] {
        let found = bvh.closest(&mesh, point).expect("the sphere has faces");
        let exact = (point.length() - radius).abs();
        assert!(
            (found.distance - exact).abs() <= sag + 1e-3,
            "at {point}: |p| - r is {exact}, the mesh gives {}",
            found.distance
        );
    }
}

#[test]
fn a_sphere_winds_once_inside_and_not_at_all_outside() {
    let radius: f32 = 20.0;
    let mesh = bodies::uv_sphere(radius, 96, 96);
    let winding = Winding::build(&mesh);

    for inside in [Vec3::ZERO, Vec3::new(19.0, 0.0, 0.0), Vec3::splat(5.0)] {
        let found = winding.at(&mesh, inside);
        assert!((found - 1.0).abs() < 0.05, "at {inside}, got {found}");
    }
    for outside in [
        Vec3::new(21.0, 0.0, 0.0),
        Vec3::new(0.0, 0.0, 200.0),
        Vec3::splat(30.0),
    ] {
        let found = winding.at(&mesh, outside);
        assert!(found.abs() < 0.05, "at {outside}, got {found}");
    }
}
