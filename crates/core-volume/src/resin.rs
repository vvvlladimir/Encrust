//! What resin a model keeps that Check warns of before a print: a cavity with no way out, and
//! a solid model a wall would save much of (ADR 0393).

use core_geometry::{Mat3, Mesh, Scalar, signed_volume};
use rayon::prelude::*;

use crate::drain::{DrainHole, HoleKind};
use crate::hollow::{HollowMode, HollowSettings};

/// Least resin hollowing must save for Check to suggest it, mm³: 10 ml.
pub const MIN_SAVED_MM3: Scalar = 10_000.0;
/// Least share of the model's resin hollowing must save for Check to suggest it.
pub const MIN_SAVED_SHARE: Scalar = 0.4;

/// Whether a cavity built by `settings` keeps its resin: grown inward, its floor closed, and
/// none of `holes` a drain through the wall. A walled or support hole drains nothing.
pub fn holds_resin(settings: &HollowSettings, holes: &[DrainHole]) -> bool {
    settings.mode == HollowMode::Internal
        && settings.open_toward.is_none()
        && !holes.iter().any(|hole| hole.kind == HoleKind::Drain)
}

/// Resin a wall of `wall_mm` would save in the solid `mesh` placed by `linear`, mm³, when it
/// is past both `MIN_SAVED_MM3` and `MIN_SAVED_SHARE` of the model.
pub fn hollowing_saves_mm3(mesh: &Mesh, linear: Mat3, wall_mm: Scalar) -> Option<Scalar> {
    let (volume_mm3, area_mm2) = placed_volume_and_area(mesh, linear);
    let saved_mm3 = raw_saving_mm3(volume_mm3, area_mm2, wall_mm);
    let worth = saved_mm3 >= f64::from(MIN_SAVED_MM3)
        && saved_mm3 >= f64::from(MIN_SAVED_SHARE) * volume_mm3;
    worth.then_some(saved_mm3 as Scalar)
}

/// The resin a wall of `wall_mm` is reckoned to save in a model of `volume_mm3` and
/// `area_mm2`, before the thresholds of [`hollowing_saves_mm3`], mm³.
pub fn raw_saving_mm3(volume_mm3: f64, area_mm2: f64, wall_mm: Scalar) -> f64 {
    // The wall is taken as surface times thickness, which a convex model never needs in
    // full, so the saving is never overstated where it counts.
    volume_mm3 - area_mm2 * f64::from(wall_mm)
}

/// The volume `mesh` encloses (mm³) and its surface (mm²), once placed by `linear`, in `f64`.
pub fn placed_volume_and_area(mesh: &Mesh, linear: Mat3) -> (f64, f64) {
    let volume_mm3 = f64::from(signed_volume(mesh).abs() * linear.determinant().abs());
    let area_x2: f64 = (0..mesh.faces.len())
        .into_par_iter()
        .filter_map(|face| mesh.triangle(face))
        .map(|triangle| {
            let [a, b, c] = [triangle.a, triangle.b, triangle.c].map(|corner| linear * corner);
            f64::from((b - a).cross(c - a).length())
        })
        .sum();
    (volume_mm3, area_x2 / 2.0)
}

#[cfg(test)]
mod tests {
    use core_geometry::{Vec3, cuboid};

    use super::*;
    use crate::drain::HoleSize;

    fn hole(kind: HoleKind) -> DrainHole {
        DrainHole {
            kind,
            ..DrainHole::sized(Vec3::ZERO, Vec3::Z, HoleSize::round(3.0, 3.0, 1.0), 0.0)
        }
    }

    #[test]
    fn a_closed_cavity_without_a_drain_hole_holds_resin_and_walled_or_support_holes_do_not_drain_it()
     {
        let closed = HollowSettings::default();
        assert!(holds_resin(&closed, &[]));
        let undrained = [hole(HoleKind::Walled), hole(HoleKind::Support)];
        assert!(holds_resin(&closed, &undrained));
        assert!(!holds_resin(&closed, &[hole(HoleKind::Drain)]));
    }

    #[test]
    fn an_open_floor_or_a_mould_lets_its_resin_out() {
        let open = HollowSettings {
            open_toward: Some(Vec3::NEG_Z),
            ..HollowSettings::default()
        };
        let mould = HollowSettings {
            mode: HollowMode::External,
            ..HollowSettings::default()
        };
        assert!(!holds_resin(&open, &[]));
        assert!(!holds_resin(&mould, &[]));
    }

    #[test]
    fn a_forty_millimetre_cube_saves_its_volume_less_its_surface_times_the_wall() {
        let cube = cuboid(Vec3::ZERO, Vec3::splat(40.0));
        let saved = hollowing_saves_mm3(&cube, Mat3::IDENTITY, 2.0).expect("70 % of 64 ml");
        assert!(
            (saved - 44_800.0).abs() < 1.0,
            "64000 - 9600 x 2 = 44800 mm³, got {saved}"
        );
    }

    #[test]
    fn a_cube_scaled_twice_saves_what_one_twice_as_large_does() {
        let cube = cuboid(Vec3::ZERO, Vec3::splat(20.0));
        let saved = hollowing_saves_mm3(&cube, Mat3::from_diagonal(Vec3::splat(2.0)), 2.0)
            .expect("placed, it is the forty millimetre cube");
        assert!(
            (saved - 44_800.0).abs() < 1.0,
            "expected 44800 mm³, got {saved}"
        );
    }

    #[test]
    fn a_small_cube_and_a_thin_plate_are_not_worth_hollowing() {
        let small = cuboid(Vec3::ZERO, Vec3::splat(20.0));
        assert_eq!(
            hollowing_saves_mm3(&small, Mat3::IDENTITY, 2.0),
            None,
            "8000 - 2400 x 2 = 3200 mm³ is under 10 ml"
        );
        let plate = cuboid(Vec3::ZERO, Vec3::new(200.0, 200.0, 4.0));
        assert_eq!(
            hollowing_saves_mm3(&plate, Mat3::IDENTITY, 2.0),
            None,
            "a 4 mm plate is all wall"
        );
    }

    #[test]
    fn a_placed_cube_has_its_closed_form_volume_and_surface() {
        let cube = cuboid(Vec3::ZERO, Vec3::splat(10.0));
        let (volume, area) = placed_volume_and_area(&cube, Mat3::from_diagonal(Vec3::splat(4.0)));
        assert!(
            (volume - 64_000.0).abs() < 1.0,
            "40³ = 64000 mm³, got {volume}"
        );
        assert!(
            (area - 9_600.0).abs() < 1.0,
            "6 x 40² = 9600 mm², got {area}"
        );
        let saved = raw_saving_mm3(volume, area, 2.0);
        assert!(
            (saved - 44_800.0).abs() < 1.0,
            "64000 - 9600 x 2, got {saved}"
        );
    }

    #[test]
    fn a_saving_under_the_share_is_not_worth_hollowing() {
        let bar = cuboid(Vec3::ZERO, Vec3::new(300.0, 12.0, 12.0));
        let (volume, area) = placed_volume_and_area(&bar, Mat3::IDENTITY);
        let saved = volume - area * 2.0;
        assert!(
            saved >= f64::from(MIN_SAVED_MM3) && saved < f64::from(MIN_SAVED_SHARE) * volume,
            "43200 - 14688 x 2 = 13824 mm³: past 10 ml, under 40 % of 43.2 ml, got {saved}"
        );
        assert_eq!(hollowing_saves_mm3(&bar, Mat3::IDENTITY, 2.0), None);
    }
}
