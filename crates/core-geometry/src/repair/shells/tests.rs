use super::*;
use crate::fixtures::cube_and_a_speck;
use crate::{Vec3, cuboid, cylinder, diagnose};

#[test]
fn a_speck_of_area_under_the_limit_goes() {
    let mut mesh = cube_and_a_speck();

    assert_eq!(drop_small_shells(&mut mesh, DEFAULT_SMALLEST_SHELL_MM2), 1);
    assert_eq!(mesh.faces, cuboid(Vec3::ZERO, Vec3::splat(10.0)).faces);
}

#[test]
fn a_speck_over_the_limit_stays() {
    let mut mesh = cube_and_a_speck();

    assert_eq!(
        drop_small_shells(&mut mesh, 0.4),
        0,
        "the speck is 0.5 mm^2"
    );
    assert_eq!(diagnose(&mesh).shells, 2);
}

#[test]
fn a_closed_pin_under_the_limit_stays() {
    // 0.2 mm across and 0.5 mm long: about 0.38 mm² of surface, and closed.
    let mut mesh = cuboid(Vec3::ZERO, Vec3::splat(10.0));
    let pin = cylinder(
        Vec3::new(20.0, 0.0, 0.0),
        Vec3::new(20.0, 0.0, 0.5),
        0.1,
        16,
    );
    mesh.append(&pin);

    assert_eq!(drop_small_shells(&mut mesh, DEFAULT_SMALLEST_SHELL_MM2), 0);
    assert_eq!(diagnose(&mesh).shells, 2);
}
