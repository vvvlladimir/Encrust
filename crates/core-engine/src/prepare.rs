//! The autopilot: what the engine's own defaults do to a model nobody tuned (ADR 0402). A
//! hollowed model is then drained by [`crate::suggest_drains`], vent included, and stood on
//! the catalogue's automatic supports, its cavity held up as well.

use core_geometry::{Mat3, MendMethod, Mesh, MeshDiagnostics, Scalar};
use core_plate::OrientPreset;
use core_volume::{
    HollowSettings, RepairMethod, hollowing_saves_mm3, placed_volume_and_area, raw_saving_mm3,
};

/// The repair a model with defects gets: the window's full mend.
pub const PREPARE_REPAIR: RepairMethod = RepairMethod::Mend(MendMethod::Repair);

/// What the orientation search weighs: every goal at once.
pub const PREPARE_ORIENT: OrientPreset = OrientPreset::Auto;

/// The wall a solid model is hollowed with, and the resin it is reckoned to save.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hollowing {
    pub wall_mm: Scalar,
    /// The estimate of ADR 0393, cubic millimetres.
    pub saves_mm3: Scalar,
}

impl Hollowing {
    /// A wall of `wall_mm` on `mesh` placed by `linear`, with the resin ADR 0393 reckons it
    /// saves there, none below zero, whether or not that is worth hollowing for.
    pub fn reckoned(mesh: &Mesh, linear: Mat3, wall_mm: Scalar) -> Self {
        let (volume_mm3, area_mm2) = placed_volume_and_area(mesh, linear);
        let saves_mm3 = raw_saving_mm3(volume_mm3, area_mm2, wall_mm).max(0.0) as Scalar;
        Self { wall_mm, saves_mm3 }
    }
}

/// Whether a model whose import found `found` is repaired before anything else is done to it.
pub fn needs_repair(found: &MeshDiagnostics) -> bool {
    !found.is_sound()
}

/// Hollow's default wall for a sound `mesh` placed by `linear`, where it saves enough resin to
/// name (ADR 0393), the cavity left empty as Hollow leaves it; `None` prints it solid.
pub fn hollowing(mesh: &Mesh, linear: Mat3, found: &MeshDiagnostics) -> Option<Hollowing> {
    if !found.is_sound() {
        return None;
    }
    let wall_mm = HollowSettings::default().thickness_mm;
    hollowing_saves_mm3(mesh, linear, wall_mm).map(|saves_mm3| Hollowing { wall_mm, saves_mm3 })
}

#[cfg(test)]
mod tests {
    use core_geometry::{Vec3, cuboid, diagnose};

    use super::*;

    fn wall_for(mesh: &Mesh) -> Option<Hollowing> {
        hollowing(mesh, Mat3::IDENTITY, &diagnose(mesh))
    }

    #[test]
    fn a_wall_is_reckoned_to_save_the_volume_less_its_surface_times_the_wall_never_below_zero() {
        let cube = cuboid(Vec3::ZERO, Vec3::splat(40.0));
        let reckoned = Hollowing::reckoned(&cube, Mat3::IDENTITY, 2.0);
        let expected_mm3 = 64_000.0 - 9_600.0 * 2.0;
        assert!(
            (reckoned.saves_mm3 - expected_mm3).abs() < 1.0,
            "saves {expected_mm3} mm³, found {}",
            reckoned.saves_mm3
        );
        let thick = Hollowing::reckoned(&cube, Mat3::IDENTITY, 30.0);
        assert_eq!(
            thick.saves_mm3, 0.0,
            "a wall thicker than the cube saves nothing"
        );
    }

    #[test]
    fn a_large_solid_cube_is_hollowed_at_the_default_wall() {
        let cube = cuboid(Vec3::ZERO, Vec3::splat(40.0));
        let found = wall_for(&cube).expect("a 40 mm cube saves most of itself");
        // 40³ less its 6 · 40² surface times the 2 mm wall.
        let expected_mm3 = 64_000.0 - 9_600.0 * 2.0;
        assert_eq!(found.wall_mm, 2.0, "Hollow's default wall");
        assert!(
            (found.saves_mm3 - expected_mm3).abs() < 1.0,
            "saves {expected_mm3} mm³, found {}",
            found.saves_mm3
        );
    }

    #[test]
    fn a_thin_plate_prints_solid() {
        let plate = cuboid(Vec3::ZERO, Vec3::new(60.0, 60.0, 3.0));
        assert_eq!(wall_for(&plate), None, "a 3 mm plate is all wall");
    }

    #[test]
    fn a_scaled_up_placement_turns_a_small_cube_worth_hollowing() {
        let cube = cuboid(Vec3::ZERO, Vec3::splat(10.0));
        assert_eq!(wall_for(&cube), None, "a 10 mm cube saves too little");
        let placed = hollowing(
            &cube,
            Mat3::from_diagonal(Vec3::splat(4.0)),
            &diagnose(&cube),
        );
        assert!(
            placed.is_some(),
            "placed at four times, it is the 40 mm cube"
        );
    }

    #[test]
    fn an_open_model_is_repaired_and_never_hollowed() {
        let mut open = cuboid(Vec3::ZERO, Vec3::splat(40.0));
        open.faces.pop();
        let found = diagnose(&open);
        assert!(needs_repair(&found), "a missing face is a hole");
        assert_eq!(hollowing(&open, Mat3::IDENTITY, &found), None);
        let closed = cuboid(Vec3::ZERO, Vec3::splat(40.0));
        assert!(!needs_repair(&diagnose(&closed)), "a closed cube is sound");
    }
}
