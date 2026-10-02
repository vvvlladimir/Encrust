use std::sync::Arc;

use anyhow::{Result, bail};
use core_geometry::{Mesh, Scalar};
use core_supports::Trapped;
use core_volume::{HoleSize, markers};

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
    /// Diameter of the mouth the next click drills, in millimetres of the plate.
    pub diameter_mm: Scalar,
    /// How far past the surface that hole reaches, millimetres.
    pub depth_mm: Scalar,
    /// Diameter of its far end as a fraction of its mouth: 1 is a cylinder.
    pub taper: Scalar,
    pub placing: Placing,
    pub job: Option<TrapJob>,
    /// Whether a check has finished since the plate last changed under it.
    pub checked: bool,
}

impl Default for DrainTool {
    fn default() -> Self {
        Self {
            // Three millimetres is what a resin slicer offers by default, and drains a
            // cavity of any size the machine can print.
            diameter_mm: 3.0,
            depth_mm: 3.0,
            taper: 1.0,
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
            diameter_mm: self.diameter_mm,
            depth_mm: self.depth_mm,
            taper: self.taper,
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
        let diameter_mm = self.diameter_mm;
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
    pub fn drill_found(&mut self, scene: &mut Scene) -> usize {
        let (diameter_mm, taper) = (self.diameter_mm, self.taper);
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

/// The pockets of resin the last drainage check found in one model, in its own space, and
/// the balls they are marked with. The check reads the slice stack, which is why these sit
/// beside the model's `ModelHollow` rather than in it; see `docs/decisions/0129`.
#[derive(Debug, Clone, Default)]
pub struct Traps {
    found: Vec<Trapped>,
    markers: Option<Arc<Mesh>>,
}

impl Traps {
    pub fn found(&self) -> &[Trapped] {
        &self.found
    }

    /// The pockets as balls, each as big as the resin it holds, or `None` for none.
    pub fn markers(&self) -> Option<&Arc<Mesh>> {
        self.markers.as_ref()
    }

    pub fn set(&mut self, found: Vec<Trapped>) {
        self.markers = markers(
            found
                .iter()
                .map(|trap| (trap.at, trap_radius_mm(trap.volume_mm3))),
        )
        .map(Arc::new);
        self.found = found;
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

/// How wide a marker a pocket of `volume_mm3` is drawn as: the radius of the ball that
/// would hold it, kept inside what can be seen and not lost on a big plate.
fn trap_radius_mm(volume_mm3: Scalar) -> Scalar {
    let ball = (volume_mm3 * 3.0 / (4.0 * std::f32::consts::PI))
        .max(0.0)
        .cbrt();
    ball.clamp(1.0, 5.0)
}
