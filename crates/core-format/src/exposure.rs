use serde::{Deserialize, Serialize};

/// An exposure that replaces the resin's own over a band of print height.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ExposureRange {
    /// Bottom of the band, millimetres above the plate. The band includes it.
    pub from_mm: f32,
    /// Top of the band, millimetres above the plate. The band stops below it.
    pub to_mm: f32,
    pub exposure_s: f32,
}

impl ExposureRange {
    pub fn new(from_mm: f32, to_mm: f32, exposure_s: f32) -> Self {
        Self {
            from_mm,
            to_mm,
            exposure_s,
        }
    }

    /// Whether a layer standing at `z_mm` is exposed by this band.
    pub fn contains(&self, z_mm: f32) -> bool {
        z_mm >= self.from_mm && z_mm < self.to_mm
    }
}

/// What a job exposes each layer for: bands of print height laid over the resin's own
/// exposure, empty when the whole stack takes the resin's.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExposurePlan {
    ranges: Vec<ExposureRange>,
}

impl ExposurePlan {
    pub fn new(ranges: Vec<ExposureRange>) -> Self {
        Self { ranges }
    }

    pub fn ranges(&self) -> &[ExposureRange] {
        &self.ranges
    }

    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    /// Exposure a layer standing at `z_mm` takes, or `None` where no band covers it.
    ///
    /// Bands may overlap and the last one given wins, so a narrow correction can be laid
    /// over a wide band instead of forcing the user to cut the wide one in two.
    pub fn exposure_at_s(&self, z_mm: f32) -> Option<f32> {
        self.ranges
            .iter()
            .rev()
            .find(|range| range.contains(z_mm))
            .map(|range| range.exposure_s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_plan_covers_nothing() {
        let plan = ExposurePlan::default();
        assert!(plan.is_empty());
        assert_eq!(plan.exposure_at_s(1.0), None);
    }

    #[test]
    fn a_band_ends_below_its_top() {
        let plan = ExposurePlan::new(vec![ExposureRange::new(2.0, 4.0, 3.0)]);
        assert_eq!(plan.exposure_at_s(2.0), Some(3.0));
        assert_eq!(plan.exposure_at_s(3.9), Some(3.0));
        assert_eq!(plan.exposure_at_s(4.0), None);
        assert_eq!(plan.exposure_at_s(1.9), None);
    }

    #[test]
    fn the_last_of_two_overlapping_bands_wins() {
        let plan = ExposurePlan::new(vec![
            ExposureRange::new(0.0, 10.0, 3.0),
            ExposureRange::new(4.0, 5.0, 6.0),
        ]);
        assert_eq!(plan.exposure_at_s(4.5), Some(6.0));
        assert_eq!(plan.exposure_at_s(5.0), Some(3.0));
    }
}
