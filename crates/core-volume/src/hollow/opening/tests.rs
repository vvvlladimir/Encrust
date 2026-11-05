use core_geometry::{Bvh, Mesh, Scalar, Vec3, cuboid};

use crate::fixtures::dumbbell;
use crate::{Cancel, HollowMode, HollowSettings, Hollowed, VolumeError, hollow};

/// A 2 mm wall on the coarsest lattice, the thin parts kept solid under `solid_under_mm`.
fn walled(mode: HollowMode, solid_under_mm: Option<Scalar>) -> HollowSettings {
    HollowSettings {
        thickness_mm: 2.0,
        mode,
        precision: 0.0,
        solid_under_mm,
        ..HollowSettings::default()
    }
}

fn hollowed(mesh: &Mesh, settings: &HollowSettings) -> Hollowed {
    hollow(mesh, &Bvh::build(mesh), settings, Cancel::never()).expect("the body hollows")
}

/// Whether a cavity face of `shell` has a corner with X inside `from..to`.
fn cavity_within(shell: &Hollowed, from: Scalar, to: Scalar) -> bool {
    shell.mesh.faces[shell.cavity.clone()].iter().any(|face| {
        face.iter()
            .any(|&corner| (from..to).contains(&shell.mesh.vertices[corner as usize].x))
    })
}

/// The dumbbell's walls at 1.5 mm, which leave its 4 mm bar a 1 mm cavity of its own.
fn dumbbell_walled(solid_under_mm: Option<Scalar>) -> HollowSettings {
    HollowSettings {
        thickness_mm: 1.5,
        ..walled(HollowMode::Internal, solid_under_mm)
    }
}

#[test]
fn a_bar_thinner_than_the_setting_stays_solid_between_hollowed_cubes() {
    let shell = hollowed(&dumbbell(), &dumbbell_walled(Some(6.0)));
    assert!(
        cavity_within(&shell, 0.0, 30.0),
        "the left cube gets a cavity"
    );
    assert!(
        cavity_within(&shell, 60.0, 90.0),
        "the right cube gets a cavity"
    );
    assert!(
        !cavity_within(&shell, 40.0, 50.0),
        "no cavity in the 4 mm bar's middle third, 40 to 50 mm"
    );
    // Each cube's cavity is 27 mm across: 19 683 mm3, less what the 3 mm ball rounds off.
    let expected = 2.0 * 27.0_f32.powi(3);
    assert!(
        (shell.cavity_mm3 - expected).abs() < expected * 0.02,
        "two 26 mm cavities, {expected} mm3, got {}",
        shell.cavity_mm3
    );
}

#[test]
fn without_the_setting_the_bar_is_hollowed_as_before() {
    let shell = hollowed(&dumbbell(), &dumbbell_walled(None));
    assert!(
        cavity_within(&shell, 0.0, 30.0) && cavity_within(&shell, 60.0, 90.0),
        "both cubes get a cavity"
    );
    assert!(
        cavity_within(&shell, 40.0, 50.0),
        "two 1.5 mm walls leave the 4 mm bar a cavity between 40 and 50 mm"
    );
}

#[test]
fn a_setting_under_every_part_changes_no_cavity() {
    let mesh = dumbbell();
    let plain = hollowed(&mesh, &walled(HollowMode::Internal, None));
    let kept = hollowed(&mesh, &walled(HollowMode::Internal, Some(3.0)));
    assert!(
        (kept.cavity_mm3 - plain.cavity_mm3).abs() < plain.cavity_mm3 * 0.01,
        "nothing is thinner than 3 mm, so {} mm3 as without it, got {}",
        plain.cavity_mm3,
        kept.cavity_mm3
    );
}

#[test]
fn a_cube_thinner_than_the_setting_gets_no_cavity_in_any_mode() {
    let cube = cuboid(Vec3::splat(0.017), Vec3::splat(40.017));
    let open_floor = HollowSettings {
        open_toward: Some(Vec3::NEG_Z),
        ..walled(HollowMode::Internal, Some(50.0))
    };
    for settings in [
        walled(HollowMode::Internal, Some(50.0)),
        walled(HollowMode::External, Some(50.0)),
        open_floor,
    ] {
        let shell = hollowed(&cube, &settings);
        assert!(
            shell.cavity.is_empty() && shell.cavity_mm3 == 0.0,
            "a 40 mm cube is thinner than 50 mm, so 0 mm3 of cavity, got {} in {:?}",
            shell.cavity_mm3,
            settings.mode
        );
    }
}

#[test]
fn a_thickness_to_keep_solid_under_must_be_positive() {
    let cube = cuboid(Vec3::ZERO, Vec3::splat(10.0));
    for asked in [0.0, -1.0, Scalar::NAN] {
        let refused = hollow(
            &cube,
            &Bvh::build(&cube),
            &walled(HollowMode::Internal, Some(asked)),
            Cancel::never(),
        );
        assert!(
            matches!(refused, Err(VolumeError::BadSolidUnder(_))),
            "{asked} mm is refused, got {refused:?}"
        );
    }
}
