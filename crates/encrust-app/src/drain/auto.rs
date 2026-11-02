//! Drain holes the engine proposes and proves, as `--drain-at auto` drills them (ADR 0396).

use std::sync::Arc;

use anyhow::{Result, bail};
use core_geometry::Scalar;

use super::DrainTool;
use crate::i18n::tr;
use crate::job::{DrainsJob, DrainsOutcome, DrainsRequest, DrainsTask, Proposed};
use crate::scene::{ObjectId, Scene};
use crate::status::Status;

/// What landing a search put in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Landed {
    pub holes: usize,
    /// Pockets no hole the search found could drain.
    pub trapped: usize,
    /// Models moved or remade while the search ran, which get nothing.
    pub moved: usize,
}

impl DrainTool {
    /// Starts looking for the holes that drain the picked models as they stand, each with
    /// a vent, as the command line's `--drain-at auto` does.
    pub fn start_auto(&mut self, scene: &Scene, layer_height_mm: Scalar) -> Result<()> {
        let tasks = scene
            .targets()
            .filter(|object| !object.mesh.is_empty())
            .map(|object| DrainsTask::of(object, self.size_for(object)))
            .collect();
        self.search(tasks, layer_height_mm)
    }

    /// The same search on `ids` alone, whatever is picked: what Prepare drains.
    pub fn start_auto_on(
        &mut self,
        scene: &Scene,
        ids: &[ObjectId],
        layer_height_mm: Scalar,
    ) -> Result<()> {
        let tasks = ids
            .iter()
            .filter_map(|id| scene.get(*id).filter(|object| !object.mesh.is_empty()))
            .map(|object| DrainsTask::of(object, self.size_for(object)))
            .collect();
        self.search(tasks, layer_height_mm)
    }

    fn search(&mut self, tasks: Vec<DrainsTask>, layer_height_mm: Scalar) -> Result<()> {
        if tasks.is_empty() {
            bail!(tr!("drain-nothing-visible"));
        }
        self.auto = Some(DrainsJob::spawn(DrainsRequest {
            tasks,
            layer_height_mm,
            vents: true,
        }));
        Ok(())
    }

    /// Gives up on the search going; nothing it found lands.
    pub fn cancel_auto(&mut self, status: &mut Status) {
        if self.auto.take().is_some() {
            *status = Status::Info(tr!("drain-auto-cancelled"));
        }
    }

    /// Lands a search that ended into the scene and the stage notice. Returns whether one
    /// is still going.
    pub(super) fn poll_auto(&mut self, scene: &mut Scene, status: &mut Status) -> bool {
        let Some(job) = self.auto.as_mut() else {
            return false;
        };
        let Some(outcome) = job.poll() else {
            return true;
        };
        self.auto = None;
        match outcome {
            DrainsOutcome::Found(found) => {
                let landed = land(scene, found);
                self.checked = landed.moved == 0;
                *status = told(landed);
            }
            DrainsOutcome::Cancelled => *status = Status::Info(tr!("drain-auto-cancelled")),
            DrainsOutcome::Failed(message) => *status = Status::Error(message),
        }
        false
    }
}

/// Drills what the search chose as clicks drill holes and marks what is left trapped as the
/// check does, all in one frame so one undo takes it back (ADR 0257).
pub fn land(scene: &mut Scene, found: Vec<Proposed>) -> Landed {
    let mut landed = Landed::default();
    for proposed in found {
        let Some(object) = scene.get_mut(proposed.id) else {
            landed.moved += 1;
            continue;
        };
        if !Arc::ptr_eq(object.uncut(), &proposed.mesh) || object.transform != proposed.transform {
            landed.moved += 1;
            continue;
        }
        for hole in proposed.holes {
            object.hollow.add_hole(hole);
            landed.holes += 1;
        }
        landed.trapped += proposed.trapped.len();
        object.traps.set(proposed.trapped);
    }
    landed
}

fn told(landed: Landed) -> Status {
    let drilled = tr!("drain-auto-drilled", count = landed.holes);
    let mut facts = Vec::new();
    if landed.trapped > 0 {
        facts.push(tr!("drain-auto-trapped", count = landed.trapped));
    }
    if landed.moved > 0 {
        facts.push(tr!("drain-auto-moved", count = landed.moved));
    }
    if facts.is_empty() {
        Status::Info(drilled)
    } else {
        Status::Noted(drilled, facts)
    }
}

#[cfg(test)]
mod tests;
