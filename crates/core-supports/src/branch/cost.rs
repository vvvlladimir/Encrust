//! What a merge saves: the trunk and foot it leaves out against the struts it adds, in the
//! closed forms of cones and cylinders (ADR 0424).

use core_geometry::{Scalar, Vec3};
use printer_profiles::SupportProfile;
use std::f32::consts::PI;

use crate::column::PLATE_Z_MM;

/// One front as a merge prices it: where it hangs from, how wide, and what it carries.
#[derive(Debug, Clone, Copy)]
pub(super) struct Priced<'a> {
    pub(super) position: Vec3,
    pub(super) radius_mm: Scalar,
    pub(super) tips: usize,
    pub(super) profile: &'a SupportProfile,
}

impl Priced<'_> {
    /// Resin of a trunk straight down from here to the plate and the foot under it, mm³.
    pub(super) fn standing_mm3(&self) -> Scalar {
        let height_mm = (self.position.z - PLATE_Z_MM).max(0.0);
        let base_mm = self
            .profile
            .carrying_radius_mm(height_mm, self.tips)
            .max(self.radius_mm);
        frustum_mm3(height_mm, self.radius_mm, base_mm) + foot_mm3(self.profile, base_mm)
    }

    /// Resin of the strut from here down to `from` and on to `meet`, mm³, at the width it
    /// is built to.
    pub(super) fn strut_mm3(&self, from: Vec3, meet: Vec3) -> Scalar {
        let drop_mm = self.position.distance(from);
        let lean_mm = from.distance(meet);
        frustum_mm3(drop_mm, self.radius_mm, self.radius_mm)
            + frustum_mm3(
                lean_mm,
                self.strut_radius_mm(lean_mm),
                self.strut_radius_mm(lean_mm),
            )
    }

    /// How wide the strut from here is built over `length_mm`: no narrower than the front.
    pub(super) fn strut_radius_mm(&self, length_mm: Scalar) -> Scalar {
        self.profile
            .strut_radius_mm(length_mm, self.tips)
            .max(self.radius_mm)
    }
}

/// Resin `a` and `b` merged at `meet`, leaning from `from_a` and `from_b`, save over each
/// standing alone, mm³; under zero the merge costs more than it saves.
pub(super) fn saving_mm3(
    (a, from_a): (Priced, Vec3),
    (b, from_b): (Priced, Vec3),
    meet: Vec3,
    under: &SupportProfile,
) -> Scalar {
    let joint_mm = a
        .strut_radius_mm(from_a.distance(meet))
        .max(b.strut_radius_mm(from_b.distance(meet)));
    let merged = Priced {
        position: meet,
        radius_mm: under.carrying_radius_mm(0.0, a.tips + b.tips).max(joint_mm),
        tips: a.tips + b.tips,
        profile: under,
    };
    let apart = a.standing_mm3() + b.standing_mm3();
    let together = a.strut_mm3(from_a, meet) + b.strut_mm3(from_b, meet) + merged.standing_mm3();
    apart - together
}

/// A cone cut square at both ends, `length_mm` long from `top_mm` to `bottom_mm` wide, mm³.
fn frustum_mm3(length_mm: Scalar, top_mm: Scalar, bottom_mm: Scalar) -> Scalar {
    PI * length_mm * (top_mm * top_mm + top_mm * bottom_mm + bottom_mm * bottom_mm) / 3.0
}

/// The pad and flare a trunk `base_mm` wide stands on, mm³; on a raft, only the flare.
fn foot_mm3(profile: &SupportProfile, base_mm: Scalar) -> Scalar {
    let lower_mm = profile.flare_lower_radius_mm().max(base_mm);
    let flare = frustum_mm3(lower_mm - base_mm, base_mm, lower_mm);
    if profile.raft.enabled {
        return flare;
    }
    let pad_mm = profile.base_radius_mm();
    flare + PI * pad_mm * pad_mm * profile.bottom.platform_thickness_mm
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::profile;

    fn at(profile: &SupportProfile, x: Scalar, z: Scalar) -> Priced<'_> {
        Priced {
            position: Vec3::new(x, 0.0, z),
            radius_mm: profile.carrying_radius_mm(0.0, 1),
            tips: 1,
            profile,
        }
    }

    #[test]
    fn a_column_is_its_cylinder_and_its_foot() {
        let profile = profile();
        let column = at(&profile, 0.0, 10.0);
        let r = profile.pillar_radius_mm();
        let lower = profile.flare_lower_radius_mm();
        let pad = profile.base_radius_mm();
        let expected = PI * r * r * 10.0
            + PI * (lower - r) * (r * r + r * lower + lower * lower) / 3.0
            + PI * pad * pad * profile.bottom.platform_thickness_mm;
        assert!(
            (column.standing_mm3() - expected).abs() < 1e-3,
            "a 10 mm column of the fixture is {expected} mm³, got {}",
            column.standing_mm3()
        );
    }

    #[test]
    fn two_tall_neighbours_save_a_trunk_and_a_foot_by_merging() {
        let profile = profile();
        let (a, b) = (at(&profile, 0.0, 30.0), at(&profile, 4.0, 30.0));
        let meet = Vec3::new(2.0, 0.0, 28.0);
        let saved = saving_mm3((a, a.position), (b, b.position), meet, &profile);
        assert!(
            saved > 0.0,
            "a merge 2 mm under 30 mm tips saves, got {saved} mm³"
        );
    }

    #[test]
    fn on_a_raft_a_merge_near_the_plate_with_long_struts_costs_more_than_it_saves() {
        // With no pad to leave out, two 11 mm struts outweigh a 9 mm trunk.
        let mut profile = profile();
        profile.raft.enabled = true;
        let (a, b) = (at(&profile, 0.0, 9.0), at(&profile, 16.0, 9.0));
        let meet = Vec3::new(8.0, 0.0, 1.0);
        let saved = saving_mm3((a, a.position), (b, b.position), meet, &profile);
        assert!(
            saved < 0.0,
            "two 11 mm struts to a 1 mm trunk cost more than a trunk, got {saved} mm³"
        );
    }
}
