use serde::{Deserialize, Serialize};

/// The second stage of a two-speed peel; zero distances leave one stage. A retract's second
/// stage is part of its whole travel; see `docs/decisions/0363-a-peel-has-two-stages.md`.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SecondStage {
    pub lift_distance_mm: f32,
    pub lift_speed_mm_min: f32,
    pub retract_distance_mm: f32,
    pub retract_speed_mm_min: f32,
    pub bottom_lift_distance_mm: f32,
    pub bottom_lift_speed_mm_min: f32,
    pub bottom_retract_distance_mm: f32,
    pub bottom_retract_speed_mm_min: f32,
}

impl SecondStage {
    /// Whether any lift or retract here moves at all.
    pub fn is_used(&self) -> bool {
        [
            self.lift_distance_mm,
            self.retract_distance_mm,
            self.bottom_lift_distance_mm,
            self.bottom_retract_distance_mm,
        ]
        .iter()
        .any(|distance| *distance > 0.0)
    }

    pub(crate) fn fields(&self) -> [(&'static str, f32); 8] {
        [
            ("second_stage.lift_distance_mm", self.lift_distance_mm),
            ("second_stage.lift_speed_mm_min", self.lift_speed_mm_min),
            ("second_stage.retract_distance_mm", self.retract_distance_mm),
            (
                "second_stage.retract_speed_mm_min",
                self.retract_speed_mm_min,
            ),
            (
                "second_stage.bottom_lift_distance_mm",
                self.bottom_lift_distance_mm,
            ),
            (
                "second_stage.bottom_lift_speed_mm_min",
                self.bottom_lift_speed_mm_min,
            ),
            (
                "second_stage.bottom_retract_distance_mm",
                self.bottom_retract_distance_mm,
            ),
            (
                "second_stage.bottom_retract_speed_mm_min",
                self.bottom_retract_speed_mm_min,
            ),
        ]
    }
}
