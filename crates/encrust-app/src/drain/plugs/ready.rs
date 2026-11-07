use super::{DrainTool, Ended};
use crate::drilling::Drilling;
use crate::scene::Scene;

impl DrainTool {
    /// The plugs cut whose model's holes are no longer being taken out: plugs land with the
    /// holes they close, and what they note comes once the run is over (ADR 0412).
    pub fn plugs_to_land(&mut self, drilling: &Drilling) -> Vec<Ended> {
        let ended = self.poll_plugs();
        self.held.extend(ended);
        let (ready, held) = std::mem::take(&mut self.held)
            .into_iter()
            .partition(|ended| !drilling.drills(ended.model));
        self.held = held;
        ready
    }

    /// Gives up the plugs being cut, held or waiting: each model keeps the plugs it has, and is
    /// not asked for those again until its holes, wall or gap change.
    pub fn give_up_plugs(&mut self, scene: &mut Scene) {
        let cut = self
            .laying
            .drain(..)
            .filter(|laying| !laying.stale)
            .map(|laying| (laying.model, laying.wanted));
        let held = self.held.drain(..).map(|held| (held.model, held.wanted));
        let waiting = self
            .waiting
            .drain(..)
            .map(|waiting| (waiting.model, waiting.wanted));
        for (model, wanted) in cut.chain(held).chain(waiting) {
            if let Some(object) = scene.get_mut(model) {
                object.plugs_laid = Some(wanted);
            }
        }
    }
}
