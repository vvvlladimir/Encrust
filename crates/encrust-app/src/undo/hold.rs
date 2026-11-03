//! A run of several landings recorded as one edit: Prepare's (ADR 0399).

use super::{Frame, History, fingerprint, put_scene_back};
use crate::scene::Scene;
use crate::slicing::Slicing;
use crate::state::Tools;
use crate::tool_settings::ToolSettings;

impl History {
    /// Records what changed before it as an edit of its own, then holds every change from here
    /// on, the plate's and the tools', for one entry recorded on the first settled frame after
    /// [`Self::release`]; undo and redo wait meanwhile.
    pub fn hold(&mut self, scene: &Scene, tools: &ToolSettings, chosen: u64) {
        let now = Frame {
            settled: true,
            typing: false,
        };
        self.observe(scene, tools, chosen, now);
        self.held = true;
    }

    /// Ends the hold: what changed under it is recorded as one entry.
    pub fn release(&mut self) {
        self.held = false;
    }

    pub fn is_held(&self) -> bool {
        self.held
    }

    /// Ends the hold by putting the plate and the tool values back as it found them, recording
    /// nothing. Tool values are left as they are once another printer or resin was chosen.
    pub fn abandon(&mut self, scene: &mut Scene, tools: &mut Tools, slicing: &mut Slicing) {
        if !std::mem::take(&mut self.held) {
            return;
        }
        if fingerprint(scene) != self.fingerprint {
            put_scene_back(scene, self.last.snapshot());
        }
        let was = self.tools.as_deref().cloned();
        if let Some(was) = was.filter(|_| slicing.chosen() == self.chosen)
            && was != ToolSettings::of(tools, slicing)
        {
            self.put_tools_back(was, tools, slicing);
        }
        self.follow_up(scene, tools);
    }
}

#[cfg(test)]
mod tests;
