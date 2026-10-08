//! Every value the tool panels hold, as one record.
//!
//! The same record is what the window remembers between runs (`prefs`), what a project
//! writes (`project::state`) and what one entry of the undo stack puts back (`undo`); see
//! `docs/decisions/0192`.

use core_engine::project::{
    Array, CutState, DrainState, Group, HollowState, SlicingState, SupportState,
};
use serde::{Deserialize, Serialize};

use crate::relief::ReliefState;
use crate::slicing::Slicing;
use crate::state::Tools;
use crate::supports::{SupportGroup, SupportTool};

/// The numbers every tool is set to, and the slicing settings the panels edit.
///
/// What a model was built with travels with the model instead: a cavity carries the wall
/// it was cut to, a drain hole the size it was drilled at.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSettings {
    pub slicing: SlicingState,
    pub supports: SupportState,
    pub hollow: HollowState,
    pub drain: DrainState,
    pub cut: CutState,
    pub relief: ReliefState,
    pub array: Array,
}

impl ToolSettings {
    /// What the tools are set to now.
    pub fn of(tools: &Tools, slicing: &Slicing) -> Self {
        Self {
            slicing: slicing_state(slicing),
            supports: support_state(&tools.supports),
            hollow: tools.hollow.state.clone(),
            drain: tools.drain.state.clone(),
            cut: tools.cut.state.clone(),
            relief: tools.relief.state,
            array: tools.array,
        }
    }

    /// Stands every tool under these values.
    pub fn apply(self, tools: &mut Tools, slicing: &mut Slicing) {
        restore_slicing(self.slicing, slicing);
        restore_supports(self.supports, &mut tools.supports);

        tools.hollow.state = self.hollow;
        tools.drain.state = self.drain;
        tools.cut.state = self.cut;
        tools.relief.state = self.relief;
        tools.array = self.array;
    }
}

fn slicing_state(slicing: &Slicing) -> SlicingState {
    SlicingState {
        layer_height_mm: slicing.layer_height_mm(),
        adaptive: slicing.adaptive,
        exposure: slicing.bands_as_measured(),
        samples: slicing.samples,
        anti_alias: slicing.anti_alias,
        grey_levels: slicing.grey_levels,
        blur_px: slicing.blur_px,
        remove_islands: slicing.remove_islands,
        format: slicing.format.into(),
    }
}

fn support_state(supports: &SupportTool) -> SupportState {
    SupportState {
        groups: supports
            .groups
            .iter()
            .zip(supports.table())
            .map(|(group, profile)| Group {
                name: group.name.clone(),
                profile,
            })
            .collect(),
        active: supports.active,
        fill: supports.fill,
        brush_radius_mm: supports.brush_radius_mm,
        flood_angle_deg: supports.flood_angle_deg,
    }
}

/// The bands are kept at the height the resin was measured at, so they go in before the
/// layer height carries them to the one that was asked for.
fn restore_slicing(state: SlicingState, slicing: &mut Slicing) {
    slicing.exposure = state.exposure;
    slicing.set_layer_height(state.layer_height_mm);
    slicing.adaptive = state.adaptive;
    slicing.samples = state.samples;
    slicing.anti_alias = state.anti_alias;
    slicing.grey_levels = state.grey_levels;
    slicing.blur_px = state.blur_px;
    slicing.remove_islands = state.remove_islands;
    slicing.format = state.format.into();
}

fn restore_supports(state: SupportState, supports: &mut SupportTool) {
    if !state.groups.is_empty() {
        supports.groups = state
            .groups
            .into_iter()
            .map(|group| SupportGroup {
                name: group.name,
                profile: group.profile,
            })
            .collect();
        supports.active = state.active.min(supports.groups.len() as u16 - 1);
        supports.profile = supports.groups[supports.active as usize].profile.clone();
    }
    supports.fill = state.fill;
    supports.brush_radius_mm = state.brush_radius_mm;
    supports.flood_angle_deg = state.flood_angle_deg;
    supports.picked.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_engine::project::{Axis, Keep};
    use core_volume::HollowMode;

    /// Tools set to something other than their defaults, one value per tool.
    fn edited() -> Tools {
        let mut tools = Tools::default();
        tools.hollow.state.thickness_mm = 3.5;
        tools.hollow.state.mode = HollowMode::External;
        tools.drain.state.diameter_mm = 4.25;
        tools.cut.state.axis = Axis::X;
        tools.cut.state.keep = Keep::Above;
        tools.relief.state.amplitude_mm = -0.75;
        tools.array.columns = 5;
        tools.supports.brush_radius_mm = 6.0;
        tools.supports.profile.name = "Heavy".to_owned();
        tools
    }

    #[test]
    fn every_tool_value_comes_back() {
        let mut slicing = Slicing::default();
        let settings = ToolSettings::of(&edited(), &slicing);

        let mut tools = Tools::default();
        settings.clone().apply(&mut tools, &mut slicing);

        assert_eq!(ToolSettings::of(&tools, &slicing), settings);
        assert_eq!(tools.supports.profile.name, "Heavy");
        assert_eq!(tools.array.columns, 5);
    }

    #[test]
    fn what_is_recorded_is_what_the_tools_are_set_to() {
        let settings = ToolSettings::of(&edited(), &Slicing::default());
        assert!((settings.hollow.thickness_mm - 3.5).abs() < f32::EPSILON);
        assert!((settings.relief.amplitude_mm + 0.75).abs() < f32::EPSILON);
        assert_eq!(settings.cut.axis, Axis::X);
    }

    #[test]
    fn the_values_survive_a_round_trip_through_json() {
        let settings = ToolSettings::of(&edited(), &Slicing::default());
        let text = serde_json::to_string(&settings).expect("the settings serialise");
        let read: ToolSettings = serde_json::from_str(&text).expect("and read back");
        assert_eq!(read, settings);
    }
}
