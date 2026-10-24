use super::{MIN_FACETS, RIGHT_ANGLE_DEG, SupportProfile};
use crate::ProfileError;

pub(super) fn order(
    smaller: &'static str,
    smaller_value: f32,
    larger: &'static str,
    larger_value: f32,
) -> Result<(), ProfileError> {
    if smaller_value > larger_value {
        return Err(ProfileError::OutOfOrder {
            smaller,
            smaller_value,
            larger,
            larger_value,
        });
    }
    Ok(())
}

/// Refuses a `max_overhang_deg` past a right angle: no surface of a print leans further.
pub(crate) fn overhang_within_right_angle(max_overhang_deg: f32) -> Result<(), ProfileError> {
    if max_overhang_deg > RIGHT_ANGLE_DEG {
        return Err(ProfileError::OutOfOrder {
            smaller: "max_overhang_deg",
            smaller_value: max_overhang_deg,
            larger: "a right angle",
            larger_value: RIGHT_ANGLE_DEG,
        });
    }
    Ok(())
}

impl SupportProfile {
    pub(super) fn validate(&self) -> Result<(), ProfileError> {
        self.validate_signs()?;
        overhang_within_right_angle(self.max_overhang_deg)?;
        if self.facets < MIN_FACETS {
            return Err(ProfileError::TooFew {
                field: "facets",
                minimum: MIN_FACETS,
                value: self.facets,
            });
        }
        self.validate_widths()
    }

    /// Sizes that must be above zero, then those that may be zero but not below it.
    pub(super) fn validate_signs(&self) -> Result<(), ProfileError> {
        let positive = [
            ("density", self.density),
            ("max_overhang_deg", self.max_overhang_deg),
            ("contact_spacing_mm", self.contact_spacing_mm),
            ("steep_contact_spacing_mm", self.steep_contact_spacing_mm),
            ("contact_diameter_mm", self.tip.contact_diameter_mm),
            ("length_mm", self.top.length_mm),
            ("diameter_mm", self.middle.diameter_mm),
            ("platform_diameter_mm", self.bottom.platform_diameter_mm),
            ("platform_thickness_mm", self.bottom.platform_thickness_mm),
            ("bottom lower_diameter_mm", self.bottom.lower_diameter_mm),
            (
                "small_pillar max_length_mm",
                self.small_pillar.max_length_mm,
            ),
            (
                "max_trunk_diameter_mm",
                self.branching.max_trunk_diameter_mm,
            ),
            (
                "max_merge_distance_mm",
                self.branching.max_merge_distance_mm,
            ),
            ("max_strut_length_mm", self.branching.max_strut_length_mm),
            (
                "max_tips_per_trunk",
                self.branching.max_tips_per_trunk as f32,
            ),
        ];
        let shares = self.islands.share.map(|share| ("islands share", share));
        let positive = positive.into_iter().chain(shares);
        if let Some((field, value)) = positive.into_iter().find(|(_, value)| *value <= 0.0) {
            return Err(ProfileError::NonPositive { field, value });
        }
        crate::auto::ascending("islands area_mm2", &self.islands.area_mm2)?;

        let non_negative = [
            ("contact_depth_mm", self.tip.contact_depth_mm),
            ("raft slope_deg", self.raft.slope_deg),
            ("bracing start_height_mm", self.bracing.start_height_mm),
            (
                "buttress pair_above_mm",
                self.bracing.buttress.pair_above_mm,
            ),
            ("buttress one_above_mm", self.bracing.buttress.one_above_mm),
            ("z_lift_mm", self.z_lift_mm),
            ("clearance_mm", self.clearance_mm),
        ];
        match non_negative.into_iter().find(|(_, value)| *value < 0.0) {
            Some((field, value)) => Err(ProfileError::NonPositive { field, value }),
            None => Ok(()),
        }
    }

    /// Every ring of a column is at least as wide as the one above it. A column that
    /// narrows downwards turns itself inside out where the two rings cross.
    pub(super) fn validate_widths(&self) -> Result<(), ProfileError> {
        // The contact is not in this chain: a head wider than the pillar under it is a
        // cone that narrows, not an inversion.
        let rings = [
            ("diameter_mm", self.middle.diameter_mm),
            (
                "max_trunk_diameter_mm",
                self.branching.max_trunk_diameter_mm,
            ),
        ];
        for pair in rings.windows(2) {
            order(pair[0].0, pair[0].1, pair[1].0, pair[1].1)?;
        }
        // A thinned pillar that is thicker than the one it replaces is not a thinning.
        order(
            "small_pillar diameter_mm",
            self.small_pillar.diameter_mm,
            "diameter_mm",
            self.middle.diameter_mm,
        )?;
        order(
            "max_trunk_diameter_mm",
            self.branching.max_trunk_diameter_mm,
            "platform_diameter_mm",
            self.bottom.platform_diameter_mm,
        )?;
        // The flare stands inside the pad, so it is no wider than it.
        order(
            "bottom lower_diameter_mm",
            self.bottom.lower_diameter_mm,
            "platform_diameter_mm",
            self.bottom.platform_diameter_mm,
        )
    }
}
