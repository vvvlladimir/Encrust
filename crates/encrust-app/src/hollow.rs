use anyhow::{Result, bail};
use core_geometry::Scalar;
use core_volume::{HollowMode, HollowSettings, InfillSettings};

use crate::job::{HollowJob, HollowOutcome, HollowRequest, hollow_tasks, rebuild_tasks};
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
    /// Whether a run has just put a cavity on the plate, which is what takes the pockets
    /// an older check found off it; see ADR 0198.
    pub(crate) hollowed: bool,
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
            hollowed: false,
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

    /// Builds again every hollow model whose cuts moved under its shell, at the numbers it
    /// was built at, and says whether a run was started. A channel is a pipe only once the
    /// cavity keeps off it, so digging one on a hollow model cannot wait for a second press.
    pub fn rebuild(&mut self, scene: &Scene) -> bool {
        let tasks = rebuild_tasks(scene);
        if tasks.is_empty() || self.job.is_some() {
            return false;
        }
        self.job = Some(HollowJob::spawn(HollowRequest { tasks }));
        true
    }

    /// Whether a cavity has been cut since this was last asked, so what an older check
    /// found is forgotten once per run rather than once per frame.
    pub fn take_hollowed(&mut self) -> bool {
        std::mem::take(&mut self.hollowed)
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
                self.hollowed = models > 0;
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

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::{Mesh, Orientation, Transform, Vec3, diagnose};
    use core_volume::Shell;
    use std::sync::Arc;

    use crate::scene::{ImportSummary, Imported};

    fn scene_with_a_tetrahedron() -> Scene {
        let mesh = Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(20.0, 0.0, 0.0),
                Vec3::new(0.0, 20.0, 0.0),
                Vec3::new(0.0, 0.0, 20.0),
            ],
            vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
        );
        let mut scene = Scene::default();
        scene.insert(Imported::new(
            "tetrahedron".to_owned(),
            Arc::new(mesh.clone()),
            Transform::default(),
            ImportSummary {
                vertices_merged: 0,
                faces_removed: 0,
                orientation: Orientation {
                    flipped_faces: 0,
                    inverted_shells: 0,
                    orientable: true,
                },
                diagnostics: diagnose(&mesh),
            },
        ));
        scene
    }

    #[test]
    fn a_finished_run_asks_once_for_the_drainage_check_that_follows_it() {
        let mut scene = scene_with_a_tetrahedron();
        let mut tool = HollowTool::default();
        let mut status = Status::default();
        tool.start(&scene).expect("a model is on the plate");
        while tool.poll(&mut scene, &mut status) {
            std::thread::yield_now();
        }

        assert!(
            tool.take_hollowed(),
            "a run put a new cavity on the plate, so what an older check found is stale"
        );
        assert!(
            !tool.take_hollowed(),
            "and it looks once per run, not once per frame"
        );
    }

    #[test]
    fn digging_a_channel_into_a_hollow_model_hollows_it_again_and_nothing_else_does() {
        let mut scene = scene_with_a_tetrahedron();
        let mut tool = HollowTool::default();
        assert!(
            !tool.rebuild(&scene),
            "a solid model has no shell to rebuild"
        );

        let object = &mut scene.objects_mut()[0];
        let built = object.hollow.asking(&tool.settings());
        object.hollow.take(Shell {
            mesh: Arc::new(Mesh::default()),
            cavity: 0..0,
            cavity_mm3: 0.0,
            voxel_mm: 0.2,
            coarsened: false,
            scale: Vec3::ONE,
            settings: built,
        });
        assert!(!tool.rebuild(&scene), "the shell is what it was built from");

        let object = &mut scene.objects_mut()[0];
        object.hollow.add_channel_point(
            Vec3::new(5.0, 5.0, 0.0),
            Vec3::NEG_Z,
            Transform::default(),
        );
        object.hollow.add_channel_point(
            Vec3::new(10.0, 5.0, 0.0),
            Vec3::NEG_Z,
            Transform::default(),
        );
        assert!(object.hollow.finish_channel(2.0, Transform::default()));
        assert!(tool.rebuild(&scene), "the channel needs its sleeve now");
        assert!(tool.job.is_some());
    }
}
