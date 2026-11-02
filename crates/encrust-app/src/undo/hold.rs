//! A run of several landings recorded as one edit: Prepare's (ADR 0397).

use super::{History, fingerprint, put_scene_back};
use crate::scene::Scene;
use crate::state::Tools;

impl History {
    /// Holds every change from here on, the plate's and the tools', for one entry recorded
    /// on the first settled frame after [`Self::release`]; undo and redo wait meanwhile.
    pub fn hold(&mut self) {
        self.held = true;
    }

    /// Ends the hold: what changed under it is recorded as one entry.
    pub fn release(&mut self) {
        self.held = false;
    }

    pub fn is_held(&self) -> bool {
        self.held
    }

    /// Ends the hold by putting the plate back as it stood when it began, recording nothing.
    /// A tool value changed meanwhile stays, to be recorded as an edit of its own.
    pub fn abandon(&mut self, scene: &mut Scene, tools: &mut Tools) {
        if !std::mem::take(&mut self.held) {
            return;
        }
        if fingerprint(scene) != self.fingerprint {
            put_scene_back(scene, self.last.snapshot());
        }
        self.follow_up(scene, tools);
    }
}

#[cfg(test)]
mod tests;
