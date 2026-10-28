use serde::{Deserialize, Serialize};

use crate::ProfileError;

/// The band along each plate edge the maker keeps models out of, millimetres in plate axes.
/// Only a warning: a model standing in it still slices whole. See ADR 0372.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PlateInset {
    pub min_x_mm: f32,
    pub max_x_mm: f32,
    pub min_y_mm: f32,
    pub max_y_mm: f32,
}

impl PlateInset {
    /// The low and high corners of what it leaves of a `width_mm` by `depth_mm` plate.
    pub fn area_mm(&self, width_mm: f32, depth_mm: f32) -> [[f32; 2]; 2] {
        [
            [self.min_x_mm, self.min_y_mm],
            [width_mm - self.max_x_mm, depth_mm - self.max_y_mm],
        ]
    }

    /// Refuses a negative band, or bands that leave nothing of a `width_mm` by `depth_mm` plate.
    pub(crate) fn validate(&self, width_mm: f32, depth_mm: f32) -> Result<(), ProfileError> {
        let bands = [
            ("plate_inset.min_x_mm", self.min_x_mm),
            ("plate_inset.max_x_mm", self.max_x_mm),
            ("plate_inset.min_y_mm", self.min_y_mm),
            ("plate_inset.max_y_mm", self.max_y_mm),
        ];
        if let Some((field, value)) = bands.into_iter().find(|(_, value)| *value < 0.0) {
            return Err(ProfileError::Negative { field, value });
        }
        let [low, high] = self.area_mm(width_mm, depth_mm);
        if low[0] >= high[0] || low[1] >= high[1] {
            return Err(ProfileError::InsetLeavesNoPlate);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_area_is_the_plate_less_each_edges_band() {
        let inset = PlateInset {
            min_x_mm: 1.47,
            max_x_mm: 1.15,
            min_y_mm: 1.3,
            max_y_mm: 0.26,
        };
        let [low, high] = inset.area_mm(82.62, 130.56);
        assert_eq!(low, [1.47, 1.3]);
        let expected = [81.47, 130.3];
        assert!(
            (high[0] - expected[0]).abs() < 1e-4 && (high[1] - expected[1]).abs() < 1e-4,
            "{high:?}, expected {expected:?}"
        );
    }

    #[test]
    fn a_negative_band_is_refused() {
        let inset = PlateInset {
            min_y_mm: -1.0,
            ..PlateInset::default()
        };
        assert!(matches!(
            inset.validate(82.62, 130.56),
            Err(ProfileError::Negative {
                field: "plate_inset.min_y_mm",
                ..
            })
        ));
    }

    #[test]
    fn bands_meeting_across_the_plate_are_refused() {
        let inset = PlateInset {
            min_x_mm: 50.0,
            max_x_mm: 40.0,
            ..PlateInset::default()
        };
        assert!(matches!(
            inset.validate(82.62, 130.56),
            Err(ProfileError::InsetLeavesNoPlate)
        ));
    }
}
