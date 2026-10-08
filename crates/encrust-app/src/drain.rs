use anyhow::{Result, bail};
use core_engine::project::DrainState;
use core_geometry::Scalar;
use core_supports::Trapped;
use core_volume::HoleSize;

use crate::job::{TrapJob, TrapOutcome, TrapRequest, trap_tasks};
use crate::scene::Scene;
use crate::status::Status;

/// What a click in the viewport puts down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Placing {
    #[default]
    Hole,
    /// Each click adds a point to the channel being laid out on the model it lands on.
    Channel,
}

/// What the Drain tool is set to, and the drainage check it has going.
///
/// The sizes are baked into each hole as it is placed, the way a blocker takes the size it
/// was dropped at, so moving a slider changes the next hole and not the ones standing.
#[derive(Debug)]
pub struct DrainTool {
    /// What the panel is set to, in the shape a project writes it down in (ADR 0191).
    pub state: DrainState,
    pub placing: Placing,
    pub job: Option<TrapJob>,
    /// Whether a check has finished since the plate last changed under it. A check is
    /// asked for by hand and never follows a run; see ADR 0198.
    pub checked: bool,
}

impl Default for DrainTool {
    fn default() -> Self {
        Self {
            state: DrainState {
                // Three millimetres is what a resin slicer offers by default, and drains
                // a cavity of any size the machine can print.
                diameter_mm: 3.0,
                depth_mm: 3.0,
                taper: 1.0,
            },
            placing: Placing::default(),
            job: None,
            checked: false,
        }
    }
}

impl DrainTool {
    /// The hole the next click drills.
    pub fn size(&self) -> HoleSize {
        HoleSize {
            diameter_mm: self.state.diameter_mm,
            depth_mm: self.state.depth_mm,
            taper: self.state.taper,
        }
    }

    /// Starts looking for resin that cannot get out of anything visible on the plate.
    pub fn start(&mut self, scene: &Scene, layer_height_mm: Scalar) -> Result<()> {
        let tasks = trap_tasks(scene);
        if tasks.is_empty() {
            bail!("nothing visible on the plate to check");
        }

        self.checked = false;
        self.job = Some(TrapJob::spawn(TrapRequest {
            tasks,
            layer_height_mm,
        }));
        Ok(())
    }

    /// Says that the plate has moved under the last check: a cavity was cut, or a hole or
    /// a channel went in or came out, so what the panel shows is no longer current.
    pub fn stale(&mut self) {
        self.checked = false;
    }

    /// Drains a running check into the scene and the status bar. Returns whether one is
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
            TrapOutcome::Checked(models) => {
                let found: usize = models.iter().map(|model| model.trapped.len()).sum();
                for model in models {
                    if let Some(object) = scene.get_mut(model.id) {
                        object.traps.set(model.trapped);
                    }
                }
                self.checked = true;
                *status = Status::Info(match found {
                    0 => "No trapped resin: everything can drain".to_owned(),
                    _ => format!("{found} pocket(s) of trapped resin"),
                });
            }
            TrapOutcome::Failed(message) => *status = Status::Error(message),
        }
        false
    }

    /// Digs the channel every model has been laying out, and says how many were dug.
    pub fn finish_channels(&mut self, scene: &mut Scene) -> usize {
        self.stale();
        let diameter_mm = self.state.diameter_mm;
        let mut dug = 0;
        for object in scene.targets_mut() {
            let transform = object.transform;
            if !object.hollow.pending().is_empty()
                && object.hollow.finish_channel(diameter_mm, transform)
            {
                dug += 1;
            }
        }
        dug
    }

    /// Drills a hole into every pocket the last check found, and says how many it put in.
    ///
    /// The marks come off with them: whether the holes drained what they were put in is
    /// the next check's answer, not this one's.
    pub fn drill_found(&mut self, scene: &mut Scene) -> usize {
        self.stale();
        let (diameter_mm, taper) = (self.state.diameter_mm, self.state.taper);
        let mut drilled = 0;

        for object in scene.targets_mut() {
            let targets: Vec<_> = object
                .traps
                .found()
                .iter()
                .map(|trapped| trapped.at)
                .collect();
            for target in targets {
                if object.hollow.drill_into(
                    &object.mesh,
                    &object.bvh,
                    target,
                    diameter_mm,
                    taper,
                    object.transform,
                ) {
                    drilled += 1;
                }
            }
            object.traps.clear();
        }
        drilled
    }
}

/// The pockets of resin the last drainage check found in one model, in its own space.
/// Each carries the box it fills, and the viewport paints the cavity red inside those
/// boxes alone, so draining one pocket takes its own red away (ADR 0200). The check reads
/// the slice stack, which is why these sit beside the model's `ModelHollow` rather than in
/// it; see `docs/decisions/0129`.
#[derive(Debug, Clone, Default)]
pub struct Traps {
    found: Vec<Trapped>,
}

impl Traps {
    pub fn found(&self) -> &[Trapped] {
        &self.found
    }

    pub fn set(&mut self, found: Vec<Trapped>) {
        self.found = found;
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::{Mesh, Orientation, Transform, Vec3, diagnose};
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
    fn a_check_runs_only_when_it_is_asked_for() {
        let mut scene = scene_with_a_tetrahedron();
        let mut tool = DrainTool::default();
        let mut status = Status::default();

        tool.stale();
        assert!(
            tool.job.is_none(),
            "a cut that moved says the last check is out of date and starts no new one"
        );

        tool.start(&scene, 0.05).expect("a closed mesh checks");
        while tool.poll(&mut scene, &mut status) {
            std::thread::yield_now();
        }
        assert!(tool.checked, "the check that was asked for answered");
    }

    #[test]
    fn a_cut_leaves_the_last_check_out_of_date() {
        let mut scene = scene_with_a_tetrahedron();
        let mut tool = DrainTool::default();
        let mut status = Status::default();

        tool.start(&scene, 0.05).expect("a closed mesh checks");
        while tool.poll(&mut scene, &mut status) {
            std::thread::yield_now();
        }
        assert!(tool.checked);

        tool.stale();
        assert!(
            !tool.checked,
            "the plate moved, so what the panel reports is no longer what it holds"
        );
    }

    #[test]
    fn an_empty_plate_has_nothing_to_check() {
        let scene = Scene::default();
        let mut tool = DrainTool::default();
        assert!(tool.start(&scene, 0.05).is_err());
    }
}
