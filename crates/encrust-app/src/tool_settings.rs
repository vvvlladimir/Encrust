//! Every value the tool panels hold, as one record.
//!
//! The same record is what the window remembers between runs (`prefs`), what a project
//! writes (`project::state`) and what one entry of the undo stack puts back (`undo`); see
//! `docs/decisions/0192`. The print settings are the undo stack's alone (ADR 0380).

use core_engine::project::{
    Array, CutState, DrainState, Group, HollowState, SlicingState, SupportState,
};
use serde::{Deserialize, Serialize};

use core_volume::{BlockerSize, Boolean};

use crate::orient::OrientState;
use crate::relief::ReliefState;
use crate::repair::RepairState;
use crate::slicing::{PrintSettings, Slicing};
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
    /// How the next Keep solid click sizes its blocker; the window's, never a project's
    /// (ADR 0397).
    #[serde(default)]
    pub keep_solid: BlockerSize,
    /// Auto-orient's clean surface and the brush that paints what shows (ADR 0410).
    #[serde(default)]
    pub orient: OrientState,
    /// The recipe Prepare for print takes, by id (ADR 0405).
    #[serde(default = "prepare_recipe")]
    pub recipe: String,
    /// What the Print settings panel holds; never written, so neither a project nor the next
    /// run takes it.
    #[serde(skip)]
    pub print: Option<PrintSettings>,
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
            keep_solid: tools.hollow.blocker,
            orient: tools.orient.state(),
            recipe: tools.prepare.recipe.clone(),
            print: Some(slicing.print_settings()),
        }
    }

    /// Stands every tool under these values.
    pub fn apply(self, tools: &mut Tools, slicing: &mut Slicing) {
        restore_slicing(self.slicing, slicing);
        if let Some(print) = self.print {
            slicing.put_print_settings(print);
        }
        restore_supports(self.supports, &mut tools.supports);

        tools.hollow.state = self.hollow;
        tools.drain.state = self.drain;
        tools.cut.state = self.cut;
        tools.relief.state = self.relief;
        tools.array = self.array;
        tools.text.state = self.text;
        tools.boolean.op = self.combine;
        tools.repair.state = self.repair;
        tools.hollow.blocker = self.keep_solid;
        tools.orient.set_state(self.orient);
        tools.prepare.recipe = self.recipe;
    }
}

fn prepare_recipe() -> String {
    crate::prepare::DEFAULT_RECIPE.to_owned()
}

fn slicing_state(slicing: &Slicing) -> SlicingState {
    SlicingState {
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

fn restore_slicing(state: SlicingState, slicing: &mut Slicing) {
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
mod tests;
