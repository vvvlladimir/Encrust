//! Every value the tool panels hold, as one record.
//!
//! The same record is what the window remembers between runs (`prefs`), what a project
//! writes (`project::state`) and what one entry of the undo stack puts back (`undo`); see
//! `docs/decisions/0192`.

use core_engine::project::{
    Array, CutState, DrainState, Group, HollowState, SlicingState, SupportState,
};
use serde::{Deserialize, Serialize};

use core_volume::Boolean;

use crate::relief::ReliefState;
use crate::repair::RepairState;
use crate::slicing::Slicing;
use crate::state::Tools;
use crate::supports::{SupportGroup, SupportTool};
use crate::text::TextState;

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
    // A record written before these tools were stays readable, on their defaults.
    #[serde(default)]
    pub text: TextState,
    #[serde(default)]
    pub combine: Boolean,
    #[serde(default)]
    pub repair: RepairState,
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
            text: tools.text.state.clone(),
            combine: tools.boolean.op,
            repair: tools.repair.state,
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
        tools.text.state = self.text;
        tools.boolean.op = self.combine;
        tools.repair.state = self.repair;
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
        hanging_passes: slicing.hanging_passes,
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
        inline_spacing_mm: supports.inline_spacing_mm,
        fill_profiles: supports.fill_profiles.clone(),
        raft: Some(supports.raft),
        sizing: supports.sizing(),
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
    slicing.hanging_passes = state.hanging_passes;
    slicing.set_format(state.format.into());
}

fn restore_supports(state: SupportState, supports: &mut SupportTool) {
    if !state.groups.is_empty() {
        // Hiding is of the view and not a setting, so a group that is still the one it
        // was stays hidden through an undo.
        let was = std::mem::take(&mut supports.groups);
        supports.groups = state
            .groups
            .into_iter()
            .enumerate()
            .map(|(index, group)| SupportGroup {
                hidden: was
                    .get(index)
                    .is_some_and(|old| old.hidden && old.name == group.name),
                name: group.name,
                profile: group.profile,
            })
            .collect();
        supports.active = state.active.min(supports.groups.len() as u16 - 1);
        supports.profile = supports.groups[supports.active as usize].profile.clone();
    }
    // A file from before the raft was the plate's kept it in its first group's profile.
    supports.raft = state.raft.unwrap_or(supports.groups[0].profile.raft);
    supports.fill = state.fill;
    supports.brush_radius_mm = state.brush_radius_mm;
    supports.flood_angle_deg = state.flood_angle_deg;
    supports.inline_spacing_mm = state.inline_spacing_mm;
    supports.fill_profiles = state.fill_profiles;
    supports.set_sizing(state.sizing);
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
        tools.text.state.family = "Serif".to_owned();
        tools.text.state.depth_mm = 1.5;
        tools.boolean.op = Boolean::Difference;
        tools.repair.state.method = core_volume::RepairMethod::Wrap;
        tools.repair.state.settings.wrap_mm = 0.35;
        tools.supports.brush_radius_mm = 6.0;
        tools.supports.inline_spacing_mm = 2.5;
        tools.supports.profile.name = "Heavy".to_owned();
        let catalogue = printer_profiles::Catalogue::bundled().expect("the bundle reads");
        tools.supports.set_automatic(true, &catalogue);
        if let Some(auto) = tools.supports.auto.as_mut() {
            auto.density = 1.75;
        }
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
        assert!((tools.supports.inline_spacing_mm - 2.5).abs() < f32::EPSILON);
        assert_eq!(tools.array.columns, 5);
        assert_eq!(tools.text.state.family, "Serif");
        assert_eq!(tools.boolean.op, Boolean::Difference);
        assert_eq!(tools.repair.state.method, core_volume::RepairMethod::Wrap);
        assert_eq!(
            tools.supports.auto.map(|auto| auto.density),
            Some(1.75),
            "automatic sizing comes back on, with its numbers as edited"
        );
    }

    #[test]
    fn automatic_sizing_turned_off_keeps_its_numbers_for_when_it_is_turned_on() {
        let catalogue = printer_profiles::Catalogue::bundled().expect("the bundle reads");
        let mut slicing = Slicing::default();
        let mut tools = edited();
        tools.supports.set_automatic(false, &catalogue);
        let settings = ToolSettings::of(&tools, &slicing);

        let mut restored = Tools::default();
        settings.apply(&mut restored, &mut slicing);
        assert!(restored.supports.auto.is_none(), "it comes back off");
        restored.supports.set_automatic(true, &catalogue);
        assert_eq!(restored.supports.auto.map(|auto| auto.density), Some(1.75));
    }

    #[test]
    fn a_hidden_group_stays_hidden_when_the_values_are_put_back() {
        let mut slicing = Slicing::default();
        let mut tools = edited();
        let settings = ToolSettings::of(&tools, &slicing);
        tools.supports.groups[0].hidden = true;
        tools.supports.brush_radius_mm = 2.0;

        settings.clone().apply(&mut tools, &mut slicing);
        assert!(tools.supports.groups[0].hidden, "the view is not a setting");
        assert!((tools.supports.brush_radius_mm - 6.0).abs() < f32::EPSILON);

        let mut renamed = settings;
        renamed.supports.groups[0].name = "Another".to_owned();
        renamed.apply(&mut tools, &mut slicing);
        assert!(
            !tools.supports.groups[0].hidden,
            "a group that is no longer the same one is shown"
        );
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

    #[test]
    fn a_record_from_before_the_text_combine_and_repair_tools_still_reads() {
        let mut written = serde_json::to_value(ToolSettings::of(&edited(), &Slicing::default()))
            .expect("the settings serialise");
        let fields = written.as_object_mut().expect("the record is an object");
        fields.remove("text");
        fields.remove("combine");
        fields.remove("repair");
        let read: ToolSettings = serde_json::from_value(written).expect("an older record reads");
        assert_eq!(read.text, TextState::default());
        assert_eq!(read.combine, Boolean::Union);
        assert_eq!(read.repair, RepairState::default());
        assert!(
            (read.hollow.thickness_mm - 3.5).abs() < f32::EPSILON,
            "and keeps the rest"
        );
    }
}
