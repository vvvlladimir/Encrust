use std::sync::Arc;

use anyhow::{Result, bail};
use core_engine::project::HollowState;
use core_geometry::{Scalar, Transform};
use core_volume::{HollowMode, HollowSettings, InfillSettings, plate_down};

use crate::i18n::tr;
use crate::job::{
    HollowJob, HollowOutcome, HollowRequest, HollowTask, SharedFields, again_tasks, hollow_tasks,
    lock_fields, rebuild_tasks,
};
use crate::scene::{ObjectId, Scene, SceneObject};
use crate::status::Status;

/// How wide a blocker a click drops, in millimetres of the plate.
pub const DEFAULT_BLOCKER_MM: Scalar = 4.0;

/// What the Hollow tool is set to, and the run it has going.
///
/// The numbers live here rather than in a profile because a wall thickness is a property
/// of the model being printed, not of the machine printing it; see
/// `docs/decisions/0059-a-hollow-is-the-model-with-its-cavity-appended.md`.
#[derive(Debug)]
pub struct HollowTool {
    /// What the panel is set to, in the shape a project writes it down in (ADR 0191).
    pub state: HollowState,
    pub job: Option<HollowJob>,
    /// Whether a run has just put a cavity on the plate, which is what takes the pockets
    /// an older check found off it; see ADR 0198.
    pub(crate) hollowed: bool,
    /// The cavity fields of the models hollowed lately, so a blocker or a channel moved on
    /// one cuts its cavity again without building the field.
    fields: SharedFields,
}

impl Default for HollowTool {
    fn default() -> Self {
        let asked = HollowSettings::default();
        Self {
            state: HollowState {
                thickness_mm: asked.thickness_mm,
                mode: asked.mode,
                precision: asked.precision,
                infill_on: false,
                infill: InfillSettings::default(),
                blocker_mm: DEFAULT_BLOCKER_MM,
                open_floor: false,
            },
            job: None,
            hollowed: false,
            fields: SharedFields::default(),
        }
    }
}

impl HollowTool {
    /// What the plate's numbers ask a run for. What each model carries — its blockers and
    /// its drain holes — is laid over this per model, in `hollow_tasks` and `is_stale`.
    #[cfg(test)]
    pub fn settings(&self) -> HollowSettings {
        settings_of(&self.state, Transform::default())
    }

    /// What `object` is hollowed with: its own settings, or the plate's, with its floor
    /// opened toward the plate as it stands now.
    pub fn settings_for(&self, object: &SceneObject) -> HollowSettings {
        let state = object.own.hollow.as_ref().unwrap_or(&self.state);
        settings_of(state, object.transform)
    }

    /// Why the Hollow button is greyed out, or `None` when it is not.
    pub fn blocker(&self, scene: &Scene) -> Option<String> {
        if scene.target_count() == 0 {
            return Some(tr!("hollow-nothing-visible"));
        }
        None
    }

    /// Starts a run over everything visible on the plate.
    pub fn start(&mut self, scene: &Scene) -> Result<()> {
        let tasks = hollow_tasks(scene, |object| self.settings_for(object));
        self.spawn(tasks)
    }

    /// Starts a run over `models` alone, each at its wall beside it, millimetres, and
    /// Hollow's defaults otherwise, whatever the panel is set to: what Prepare hollows.
    pub fn start_walls(&mut self, scene: &Scene, models: &[(ObjectId, Scalar)]) -> Result<()> {
        let tasks = models
            .iter()
            .filter_map(|(id, wall_mm)| {
                let object = scene.get(*id).filter(|object| !object.mesh.is_empty())?;
                Some(HollowTask::of(object, &HollowSettings::for_wall(*wall_mm)))
            })
            .collect();
        self.spawn(tasks)
    }

    fn spawn(&mut self, tasks: Vec<HollowTask>) -> Result<()> {
        if tasks.is_empty() {
            bail!(tr!("hollow-nothing-visible-error"));
        }
        self.job = Some(HollowJob::spawn(HollowRequest {
            tasks,
            fields: Arc::clone(&self.fields),
        }));
        Ok(())
    }

    /// Builds again every hollow model whose cuts moved under its shell, at the numbers it
    /// was built at, and says whether a run was started. A channel is a pipe only once the
    /// cavity keeps off it, so digging one on a hollow model cannot wait for a second press.
    pub fn rebuild(&mut self, scene: &Scene) -> bool {
        let tasks = rebuild_tasks(scene);
        if tasks.is_empty() || self.job.is_some() {
            return false;
        }
        self.job = Some(HollowJob::spawn(HollowRequest {
            tasks,
            fields: Arc::clone(&self.fields),
        }));
        true
    }

    /// Hollows again, each at the numbers it was built at, the models a relief was pressed
    /// into. Says whether it took them, which it cannot while a run is going.
    pub fn again(&mut self, scene: &Scene, models: &[(ObjectId, HollowSettings)]) -> bool {
        if self.job.is_some() {
            return false;
        }
        let tasks = again_tasks(scene, models);
        if !tasks.is_empty() {
            self.job = Some(HollowJob::spawn(HollowRequest {
                tasks,
                fields: Arc::clone(&self.fields),
            }));
        }
        true
    }

    /// Lets the kept fields go where they must make way for a plate being sliced.
    pub fn make_way(&self) {
        lock_fields(&self.fields).make_way();
    }

    /// Whether a cavity has been cut since this was last asked, so what an older check
    /// found is forgotten once per run rather than once per frame.
    pub fn take_hollowed(&mut self) -> bool {
        std::mem::take(&mut self.hollowed)
    }

    /// Drains a running job into the scene and the stage notice. Returns whether one is
    /// still going, which is what tells the window to keep repainting.
    pub fn poll(&mut self, scene: &mut Scene, status: &mut Status) -> bool {
        let Some(job) = self.job.as_mut() else {
            return false;
        };
        let Some(outcome) = job.poll() else {
            return true;
        };

        self.job = None;
        let saved = outcome.saved_mm3();
        match outcome {
            HollowOutcome::Hollowed(shells) => {
                let models = shells.len();
                self.hollowed = models > 0;
                for shell in shells {
                    if let Some(object) = scene.get_mut(shell.id) {
                        object.hollow.take(shell.shell);
                    }
                }
                *status = Status::Info(tr!(
                    "hollow-done",
                    count = models,
                    saved = format!("{:.1}", saved / 1000.0)
                ));
            }
            HollowOutcome::Cancelled => *status = Status::Info(tr!("hollow-cancelled")),
            HollowOutcome::Failed(message) => *status = Status::Error(message),
        }
        false
    }
}

/// The run settings a panel's numbers ask for, on a model standing under `transform`. An
/// open floor faces the plate it stands on, so turning such a model makes its shell stale.
fn settings_of(state: &HollowState, transform: Transform) -> HollowSettings {
    let opens = state.open_floor && state.mode == HollowMode::Internal;
    HollowSettings {
        thickness_mm: state.thickness_mm,
        mode: state.mode,
        precision: state.precision,
        infill: state.infill_on.then_some(state.infill),
        open_toward: opens.then(|| plate_down(transform)),
        ..HollowSettings::default()
    }
}

#[cfg(test)]
mod tests;
