use anyhow::{Result, bail};
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
    /// Whether the plate is waiting for a check: a cavity or a cut has moved and the
    /// pockets on screen are no longer what the models hold. See ADR 0189.
    pub(crate) asked: bool,
    /// How many pockets the last check found, and whether that went from none to some,
    /// which is what turns the viewport's x-ray on.
    pub(crate) found: usize,
    pub(crate) appeared: bool,
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
            asked: false,
            found: 0,
            appeared: false,
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
        self.asked = false;
        self.job = Some(TrapJob::spawn(TrapRequest {
            tasks,
            layer_height_mm,
        }));
        Ok(())
    }

    /// Remembers what a check found, and notices the turn from none to some: only that
    /// turn opens the x-ray, so a view closed by hand is not opened again by every check
    /// after it.
    fn remember(&mut self, found: usize) {
        self.appeared = found > 0 && self.found == 0;
        self.found = found;
    }

    /// Whether the last check was the one that found resin with no way out, after one
    /// that found none. Asked once, so the view it opens can then be closed.
    pub fn take_appeared(&mut self) -> bool {
        std::mem::take(&mut self.appeared)
    }

    /// Says that the plate has moved under the last check: a cavity was cut, or a hole or
    /// a channel went in or came out.
    pub fn ask_for_a_check(&mut self) {
        self.asked = true;
    }

    /// Starts the check the plate is waiting for, and says whether one is now going.
    ///
    /// A change while a check is running is kept rather than queued, so clicking hole
    /// after hole costs one check after the one in flight and not one each.
    pub fn start_if_asked(&mut self, scene: &Scene, layer_height_mm: Scalar) -> bool {
        if !self.asked || self.job.is_some() {
            return false;
        }
        if trap_tasks(scene).is_empty() {
            self.asked = false;
            return false;
        }
        self.start(scene, layer_height_mm).is_ok()
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
                self.remember(found);
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
        self.ask_for_a_check();
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
    ///
    /// The check runs again after it, which is what takes the marks off the pockets the
    /// holes have drained and leaves the ones they have not.
    pub fn drill_found(&mut self, scene: &mut Scene) -> usize {
        self.ask_for_a_check();
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

/// The pockets of resin the last drainage check found in one model, in its own space.
/// Nothing is drawn per pocket: the viewport paints the whole cavity they stand in, which
/// is the space a hole has to let out. The check reads the slice stack, which is why these
/// sit beside the model's `ModelHollow` rather than in it; see `docs/decisions/0129`.
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
    fn a_cut_that_moved_is_checked_once_however_many_moved_while_one_ran() {
        let mut scene = scene_with_a_tetrahedron();
        let mut tool = DrainTool::default();
        let mut status = Status::default();

        assert!(
            !tool.start_if_asked(&scene, 0.05),
            "nothing has moved, so nothing is checked"
        );

        tool.ask_for_a_check();
        assert!(tool.start_if_asked(&scene, 0.05), "a cut moved");
        tool.ask_for_a_check();
        assert!(
            !tool.start_if_asked(&scene, 0.05),
            "the check in flight is the one that answers for it"
        );
        while tool.poll(&mut scene, &mut status) {
            std::thread::yield_now();
        }
        assert!(
            tool.start_if_asked(&scene, 0.05),
            "and what moved under it is checked after it"
        );
    }

    #[test]
    fn only_the_turn_from_nothing_trapped_to_something_opens_the_view() {
        let mut tool = DrainTool::default();

        tool.remember(0);
        assert!(
            !tool.take_appeared(),
            "nothing is trapped, nothing to look at"
        );

        tool.remember(2);
        assert!(
            tool.take_appeared(),
            "resin with no way out opens the model up"
        );
        assert!(!tool.take_appeared(), "and is reported once");

        tool.remember(3);
        assert!(
            !tool.take_appeared(),
            "a view closed by hand stays closed while the pockets are the same trouble"
        );

        tool.remember(0);
        tool.remember(1);
        assert!(
            tool.take_appeared(),
            "trouble that came back opens it again"
        );
    }

    #[test]
    fn an_empty_plate_forgets_the_check_it_was_asked_for() {
        let scene = Scene::default();
        let mut tool = DrainTool::default();
        tool.ask_for_a_check();
        assert!(!tool.start_if_asked(&scene, 0.05));
        assert!(!tool.asked, "there is nothing left to check it against");
    }
}
