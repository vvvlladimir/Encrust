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

#[test]
fn a_cube_and_a_speck_list_as_a_closed_shell_of_six_hundred_square_mm_and_an_open_one() {
    let listed = list_shells(&cube_and_a_speck());
    let [cube, speck] = listed.as_slice() else {
        panic!("two shells, got {}", listed.len());
    };
    assert!(cube.closed && !speck.closed);
    assert_eq!((cube.faces.len(), speck.faces.len()), (12, 1));
    assert!(
        (cube.area_mm2 - 600.0).abs() < 1e-3,
        "six 10 mm sides, 600 mm²; got {}",
        cube.area_mm2
    );
    assert!(
        (speck.area_mm2 - 0.5).abs() < 1e-6,
        "got {}",
        speck.area_mm2
    );
}

#[test]
fn removing_a_shell_by_its_number_leaves_the_others() {
    let mut mesh = cube_and_a_speck();
    assert_eq!(
        remove_shells(&mut mesh, &[0, 7]),
        1,
        "there is no eighth shell"
    );
    let left = list_shells(&mesh);
    assert_eq!(left.len(), 1);
    assert!(!left[0].closed, "the cube went and the speck stayed");
}
