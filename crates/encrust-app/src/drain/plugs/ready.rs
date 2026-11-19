use super::{DrainTool, Ended};
use crate::drilling::Drilling;
use crate::scene::Scene;
use crate::state::Doc;
use crate::status::Status;

/// Lays the plugs a worker has cut into the edit that asked for them; answers whether any is
/// still being cut.
pub fn land_ended(doc: &mut Doc, drain: &mut DrainTool, status: &mut Status) -> bool {
    let plate_mm = core_geometry::Vec2::new(doc.plate.x_mm, doc.plate.y_mm);
    for ended in drain.plugs_to_land(&doc.drilling) {
        doc.history.land(&mut doc.scene, ended.asked_by(), |scene| {
            DrainTool::land_plugs(scene, &ended, plate_mm, status);
        });
    }
    drain.laying_plugs()
}

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
