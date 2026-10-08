use anyhow::{Result, bail};
use core_geometry::{Scalar, Vec3};
use core_volume::ReliefSettings;
use serde::{Deserialize, Serialize};

use crate::job::{ReliefJob, ReliefOutcome, ReliefRequest, relief_tasks};
use crate::scene::Scene;
use crate::status::Status;

/// What the Relief tool presses with. It is the window's own record: the project manifest
/// has no entry for this tool (ADR 0191).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ReliefState {
    /// How far the surface moves where the texture is white, millimetres. Negative sinks
    /// the relief in rather than raising it.
    pub amplitude_mm: Scalar,
    pub precision: Scalar,
}

/// What the Relief tool is set to, and the run it has going.
///
/// A texture is pressed into the model as geometry rather than written as exposure: grey
/// controls cure depth, so it could only texture a face pointing up or down; see
/// `docs/decisions/0116-a-texture-is-pressed-into-the-field.md`.
#[derive(Debug)]
pub struct ReliefTool {
    pub state: ReliefState,
    pub job: Option<ReliefJob>,
}

impl Default for ReliefTool {
    fn default() -> Self {
        let asked = ReliefSettings::default();
        Self {
            state: ReliefState {
                amplitude_mm: asked.amplitude_mm,
                precision: asked.precision,
            },
            job: None,
        }
    }
}

impl ReliefTool {
    pub fn settings(&self) -> ReliefSettings {
        ReliefSettings {
            amplitude_mm: self.state.amplitude_mm,
            precision: self.state.precision,
            ..ReliefSettings::default()
        }
    }

    /// Why the Press button is greyed out, or `None` when it is not.
    pub fn blocker(&self, scene: &Scene) -> Option<&'static str> {
        if scene.target_count() == 0 {
            return Some("Nothing visible on the plate to press a texture into.");
        }
        if relief_tasks(scene).is_empty() {
            return Some(
                "None of these models carries a texture. Open an OBJ with its image \
                 beside it, or a 3MF with one inside it.",
            );
        }
        None
    }

    /// Starts a run over everything visible on the plate that carries a texture.
    pub fn start(&mut self, scene: &Scene) -> Result<()> {
        let tasks = relief_tasks(scene);
        if tasks.is_empty() {
            bail!("nothing on the plate carries a texture to press in");
        }

        self.job = Some(ReliefJob::spawn(ReliefRequest {
            tasks,
            settings: self.settings(),
        }));
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
        match outcome {
            ReliefOutcome::Pressed(models) => {
                let count = models.len();
                let coarsened = models.iter().filter(|model| model.coarsened).count();
                for model in models {
                    if let Some(object) = scene.get_mut(model.id) {
                        // The relief was pressed at printed size, so the scale is in the
                        // mesh now and must not be applied twice.
                        object.reshape(model.mesh);
                        object.transform.scale = Vec3::ONE;
                    }
                }
                let depth = self.state.amplitude_mm;
                *status = match coarsened {
                    0 => Status::Info(format!(
                        "Pressed {depth:.2} mm of relief into {count} model(s)"
                    )),
                    _ => Status::Info(format!(
                        "Pressed {depth:.2} mm of relief into {count} model(s); {coarsened} came                          out on a coarser lattice than precision asked for"
                    )),
                };
            }
            ReliefOutcome::Failed(message) => *status = Status::Error(message),
        }
        false
    }

    pub fn is_busy(&self) -> bool {
        self.job.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_plate_says_there_is_nothing_to_press_into() {
        let mut tool = ReliefTool::default();
        let scene = Scene::default();
        assert_eq!(
            tool.blocker(&scene),
            Some("Nothing visible on the plate to press a texture into.")
        );
        assert!(tool.start(&scene).is_err());
    }

    #[test]
    fn the_settings_are_what_the_panel_is_set_to() {
        let tool = ReliefTool {
            state: ReliefState {
                amplitude_mm: -0.3,
                precision: 0.8,
            },
            job: None,
        };
        let settings = tool.settings();
        assert!((settings.amplitude_mm + 0.3).abs() < 1e-6);
        assert!((settings.precision - 0.8).abs() < 1e-6);
    }
}
