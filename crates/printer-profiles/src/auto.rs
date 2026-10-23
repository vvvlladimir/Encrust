use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{IslandSpacing, ProfileError, SupportProfile};

/// How many shapes an automatic profile sizes supports into.
pub const AUTO_SIZES: usize = 3;

/// Automatic placement that sizes each support by what it holds: where supports go, and
/// which of a few support profiles each one is built to. See `docs/decisions/0325`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutoProfile {
    pub name: String,
    /// The support profiles a support is built to, by catalogue id, lightest first.
    pub shapes: [String; AUTO_SIZES],
    /// Where supports go, read as the fields of the same name on a [`SupportProfile`].
    pub density: f32,
    pub max_overhang_deg: f32,
    pub contact_spacing_mm: f32,
    pub steep_contact_spacing_mm: f32,
    /// The least island, mm² on the layer it begins on, held by the second shape and by
    /// the third, and spaced as [`IslandSpacing`] says from there.
    pub island_area_mm2: [f32; AUTO_SIZES - 1],
    /// The least model standing above an overhang, mm³, held by the second shape and by
    /// the third.
    pub overhang_volume_mm3: [f32; AUTO_SIZES - 1],
    /// The least resin a step puts out in its one layer, mm³, to be held as a step at
    /// all, then by the second shape and by the third. See `docs/decisions/0326`.
    pub step_volume_mm3: [f32; AUTO_SIZES],
    /// How far above a step of each size the faces near it are held as part of it, mm.
    pub step_height_mm: [f32; AUTO_SIZES],
    /// How far from vertical a face near a step may lean before it is held, degrees: less
    /// than the overhang angle, since the step is what it would sag from.
    pub step_lean_deg: f32,
    /// The support profile, by catalogue id, a support that steadies a part is built to:
    /// the lightest of all. See `docs/decisions/0327`.
    pub steady_shape: String,
    /// How far from vertical a face may lean before a part is steadied on it, degrees:
    /// less than a step's angle, since nothing here would sag without it.
    pub steady_lean_deg: f32,
    /// How far apart the supports that steady a part go, mm.
    pub steady_spacing_mm: f32,
    /// The least slope worth steadying a part on, mm² of it.
    pub steady_area_mm2: f32,
}

impl AutoProfile {
    /// Which shape holds an island of `area_mm2`, as an index into `shapes`.
    pub fn island_size(&self, area_mm2: f32) -> usize {
        size_of(&self.island_area_mm2, area_mm2)
    }

    /// Which shape holds an overhang with `carried_mm3` of model above it.
    pub fn overhang_size(&self, carried_mm3: f32) -> usize {
        size_of(&self.overhang_volume_mm3, carried_mm3)
    }

    /// Which shape holds a step that puts out `new_mm3` in its layer, or `None` for one too
    /// small to be held as a step.
    pub fn step_size(&self, new_mm3: f32) -> Option<usize> {
        let reached = size_of(&self.step_volume_mm3, new_mm3);
        reached.checked_sub(1)
    }

    /// Every support profile this one builds to, by catalogue id: its sizes lightest
    /// first, then the one that steadies a part.
    pub fn every_shape(&self) -> impl Iterator<Item = &String> {
        self.shapes
            .iter()
            .chain(std::iter::once(&self.steady_shape))
    }

    /// `shape` placing by this profile's numbers: what a run of it is handed.
    pub fn placing_with(&self, shape: &SupportProfile) -> SupportProfile {
        SupportProfile {
            density: self.density,
            max_overhang_deg: self.max_overhang_deg,
            contact_spacing_mm: self.contact_spacing_mm,
            steep_contact_spacing_mm: self.steep_contact_spacing_mm,
            islands: IslandSpacing {
                area_mm2: self.island_area_mm2,
                ..shape.islands
            },
            ..shape.clone()
        }
    }

    /// Takes where supports go from `placing`, as [`AutoProfile::placing_with`] put it.
    pub fn take_placing(&mut self, placing: &SupportProfile) {
        self.density = placing.density;
        self.max_overhang_deg = placing.max_overhang_deg;
        self.contact_spacing_mm = placing.contact_spacing_mm;
        self.steep_contact_spacing_mm = placing.steep_contact_spacing_mm;
    }

    pub fn from_toml_str(source: &str, path: &Path) -> Result<Self, ProfileError> {
        let profile: Self = toml::from_str(source).map_err(|source| ProfileError::Parse {
            path: path.to_owned(),
            source,
        })?;
        profile.validate()?;
        Ok(profile)
    }

    fn validate(&self) -> Result<(), ProfileError> {
        let positive = [
            ("density", self.density),
            ("max_overhang_deg", self.max_overhang_deg),
            ("contact_spacing_mm", self.contact_spacing_mm),
            ("steep_contact_spacing_mm", self.steep_contact_spacing_mm),
            ("step_lean_deg", self.step_lean_deg),
            ("steady_lean_deg", self.steady_lean_deg),
            ("steady_spacing_mm", self.steady_spacing_mm),
        ];
        if let Some((field, value)) = positive.into_iter().find(|(_, value)| *value <= 0.0) {
            return Err(ProfileError::NonPositive { field, value });
        }
        ascending("island_area_mm2", &self.island_area_mm2)?;
        ascending("overhang_volume_mm3", &self.overhang_volume_mm3)?;
        ascending("steady_area_mm2", &[self.steady_area_mm2])?;
        ascending("step_volume_mm3", &self.step_volume_mm3)?;
        ascending("step_height_mm", &self.step_height_mm)
    }
}

/// How many of `least` a measure of `value` reaches, which is the shape it is held by.
fn size_of(least: &[f32], value: f32) -> usize {
    least.iter().take_while(|least| value >= **least).count()
}

/// Thresholds that are not negative and do not fall.
pub(crate) fn ascending(field: &'static str, least: &[f32]) -> Result<(), ProfileError> {
    if let Some(value) = least.iter().copied().find(|value| *value < 0.0) {
        return Err(ProfileError::Negative { field, value });
    }
    match least.windows(2).find(|pair| pair[0] > pair[1]) {
        Some(pair) => Err(ProfileError::OutOfOrder {
            smaller: field,
            smaller_value: pair[0],
            larger: field,
            larger_value: pair[1],
        }),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHIPPED: &str = include_str!("../../../assets/profiles/auto/default.toml");

    fn shipped() -> AutoProfile {
        AutoProfile::from_toml_str(SHIPPED, Path::new("auto/default.toml"))
            .expect("the shipped profile reads")
    }

    fn with(edit: &str) -> Result<AutoProfile, ProfileError> {
        let source = SHIPPED.replace("island_area_mm2 = [2.4, 3.2]", edit);
        AutoProfile::from_toml_str(&source, Path::new("auto/odd.toml"))
    }

    #[test]
    fn a_measure_is_held_by_the_last_shape_it_reaches() {
        let auto = shipped();
        let sizes: Vec<usize> = [0.0, 2.39, 2.4, 3.1, 3.2, 50.0]
            .into_iter()
            .map(|area| auto.island_size(area))
            .collect();
        assert_eq!(
            sizes,
            [0, 0, 1, 1, 2, 2],
            "thresholds 2.4 and 3.2 mm² as shipped"
        );
        assert_eq!(auto.overhang_size(999.0), 0);
        assert_eq!(auto.overhang_size(1500.0), 1);
        assert_eq!(auto.overhang_size(2000.0), 2);
    }

    #[test]
    fn placing_numbers_go_onto_a_shape_and_come_back() {
        let mut auto = shipped();
        let mut placing =
            auto.placing_with(&SupportProfile::shipped("heavy").expect("the heavy preset ships"));
        assert_eq!(placing.contact_spacing_mm, 1.8);
        assert_eq!(
            placing.middle,
            SupportProfile::shipped("heavy")
                .expect("the heavy preset ships")
                .middle,
            "the shape is kept"
        );
        placing.density = 2.0;
        auto.take_placing(&placing);
        assert_eq!(auto.density, 2.0);
    }

    #[test]
    fn a_step_too_small_is_no_step_and_a_larger_one_takes_a_heavier_shape() {
        let auto = shipped();
        let sizes: Vec<Option<usize>> = [0.05, 0.1, 6.9, 7.0, 12.0, 50.0]
            .into_iter()
            .map(|mm3| auto.step_size(mm3))
            .collect();
        assert_eq!(
            sizes,
            [None, Some(0), Some(0), Some(1), Some(2), Some(2)],
            "thresholds 0.1, 7 and 12 mm³ as shipped"
        );
    }

    #[test]
    fn thresholds_that_fall_are_out_of_order() {
        assert!(matches!(
            with("island_area_mm2 = [3.2, 2.4]"),
            Err(ProfileError::OutOfOrder { .. })
        ));
    }

    #[test]
    fn a_negative_threshold_is_refused() {
        assert!(matches!(
            with("island_area_mm2 = [-1.0, 2.4]"),
            Err(ProfileError::Negative { .. })
        ));
    }

    #[test]
    fn a_spacing_of_zero_is_refused() {
        let source = SHIPPED.replace("contact_spacing_mm = 1.8", "contact_spacing_mm = 0.0");
        assert!(matches!(
            AutoProfile::from_toml_str(&source, Path::new("auto/odd.toml")),
            Err(ProfileError::NonPositive { .. })
        ));
    }
}
