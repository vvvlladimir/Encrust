use anyhow::{Result, bail};
use core_geometry::Scalar;
use core_volume::{HollowMode, HollowSettings, InfillSettings};

use crate::job::{HollowJob, HollowOutcome, HollowRequest, hollow_tasks};
use crate::scene::Scene;
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
    pub thickness_mm: Scalar,
    pub mode: HollowMode,
    pub precision: Scalar,
    pub infill_on: bool,
    pub infill: InfillSettings,
    /// Radius of the blocker the next click drops.
    pub blocker_mm: Scalar,
    pub job: Option<HollowJob>,
}

impl Default for HollowTool {
    fn default() -> Self {
        let asked = HollowSettings::default();
        Self {
            thickness_mm: asked.thickness_mm,
            mode: asked.mode,
            precision: asked.precision,
            infill_on: false,
            infill: InfillSettings::default(),
            blocker_mm: DEFAULT_BLOCKER_MM,
            job: None,
        }
    }
}

impl HollowTool {
    /// What the tool is asking for. What each model carries — its blockers and its drain
    /// holes — is laid over this per model, in `hollow_tasks` and in `is_stale`.
    pub fn settings(&self) -> HollowSettings {
        HollowSettings {
            thickness_mm: self.thickness_mm,
            mode: self.mode,
            precision: self.precision,
            infill: self.infill_on.then_some(self.infill),
            ..HollowSettings::default()
        }
    }

    /// Why the Hollow button is greyed out, or `None` when it is not.
    pub fn blocker(&self, scene: &Scene) -> Option<&'static str> {
        if scene.target_count() == 0 {
            return Some("Nothing visible on the plate to hollow.");
        }
        None
    }

    /// Starts a run over everything visible on the plate.
    pub fn start(&mut self, scene: &Scene) -> Result<()> {
        let tasks = hollow_tasks(scene, &self.settings());
        if tasks.is_empty() {
            bail!("nothing visible on the plate to hollow");
        }

        self.job = Some(HollowJob::spawn(HollowRequest { tasks }));
        Ok(())
    }

    /// Drains a running job into the scene and the status bar. Returns whether one is
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
                for shell in shells {
                    if let Some(object) = scene.get_mut(shell.id) {
                        object.hollow.take(shell.shell);
                    }
                }
                *status = Status::Info(match models {
                    0 => "Nothing on the plate to hollow".to_owned(),
                    _ => format!(
                        "Hollowed {models} model(s), saving {:.1} ml",
                        saved / 1000.0
                    ),
                });
            }
            HollowOutcome::Cancelled => *status = Status::Info("Hollowing cancelled".to_owned()),
            HollowOutcome::Failed(message) => *status = Status::Error(message),
        }
        false
    }
}
