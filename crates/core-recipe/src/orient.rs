//! The orient step's goal word as the plate's search preset.

use core_plate::{OrientGoal, OrientPreset};

/// The preset an orient step's `goal` names; any other word is `auto`.
pub fn preset_of(goal: &str) -> OrientPreset {
    match goal {
        "fewest-supports" => OrientGoal::FewestSupports.into(),
        "least-peel" => OrientGoal::LeastPeel.into(),
        "shortest-print" => OrientGoal::ShortestPrint.into(),
        _ => OrientPreset::Auto,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_goal_word_names_its_goal_and_an_unknown_one_is_auto() {
        assert_eq!(preset_of("least-peel"), OrientGoal::LeastPeel.into());
        assert_eq!(preset_of("whatever"), OrientPreset::Auto);
    }
}
