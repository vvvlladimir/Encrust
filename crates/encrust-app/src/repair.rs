//! What the window does about a model that arrived broken: the offer at the foot of the
//! stage, and the runs that close the holes.
//!
//! Welding, orientation and dropping the faces laid over others are done on import without
//! asking, because none of them changes what the model encloses (ADR 0005, 0270). Closing a
//! hole invents surface that was never in the file, so it is offered without blocking
//! anything and waits for the Repair button; see
//! `docs/decisions/0270-repair-cancels-what-encloses-nothing-and-never-worsens.md`.

use std::sync::Arc;

use core_geometry::MendMethod;
use core_volume::{RepairMethod, RepairSettings};
use serde::{Deserialize, Serialize};

use crate::i18n::tr;
use crate::job::{Counted, RepairJob, RepairOutcome, RepairRequest, Repaired};
use crate::scene::{ObjectId, Scene, SceneObject, Taken};
use crate::status::Status;

mod found;
mod hand;
mod pending;
mod shells;

pub use found::{Finding, Found};
pub use hand::{Hand, HandEdit};
pub use shells::{ShellRow, Shells};

/// The Repair tool: what it is set to, and the mode its card is in.
#[derive(Debug, Default)]
pub struct RepairTool {
    pub state: RepairState,
    pub mode: RepairMode,
    pub picking: Picking,
    /// The brush has the pointer: the press landed on a model.
    pub stroke: bool,
    /// Where the brush was last put down, so a drag picks along the line it drew.
    pub last_daub: Option<core_geometry::Vec2>,
}

/// Whether the Repair card runs the repairs on whole models or edits their faces by hand.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepairMode {
    #[default]
    Model,
    Faces,
}

/// What a click picks in the Faces mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Picking {
    /// Faces, under a brush or a whole surface at a time, to turn or drop.
    #[default]
    Faces,
    /// Open edges, two of them to bridge.
    Edges,
}

/// What the Repair tool is set to: the repair its button runs, the lengths it works to,
/// and the brush the Faces mode picks with.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RepairState {
    pub method: RepairMethod,
    pub settings: RepairSettings,
    /// How far the brush reaches, mm.
    pub brush_radius_mm: f32,
    /// How far a surface may turn and still be picked whole, degrees.
    pub flood_angle_deg: f32,
}

impl Default for RepairState {
    fn default() -> Self {
        Self {
            method: RepairMethod::default(),
            settings: RepairSettings::default(),
            brush_radius_mm: 2.0,
            flood_angle_deg: 20.0,
        }
    }
}

impl RepairState {
    /// What the Repair button of a model's row and of the stage notice runs: the plain
    /// repair, to the lengths the tool is set to.
    pub fn plain(self) -> Self {
        Self {
            method: RepairMethod::Mend(MendMethod::Repair),
            ..self
        }
    }
}

/// The repairs under way, and the broken models the user chose to keep as they came.
#[derive(Debug, Default)]
pub struct Repairs {
    /// Each with the model it took, so that a mesh replaced since is not mended over.
    jobs: Vec<(RepairJob, Taken)>,
    kept: Vec<ObjectId>,
    /// Where the Repair tool's findings lie on the models.
    pub found: Found,
    /// The Faces mode's picks and its changes by hand.
    pub hand: Hand,
    /// The shells of the one model the tool works on.
    pub shells: Shells,
}

impl Repairs {
    /// The broken models on the plate being edited that repair is still offered for:
    /// neither kept as they came nor being repaired already.
    pub fn offered<'a>(&'a self, scene: &'a Scene) -> impl Iterator<Item = &'a SceneObject> {
        scene.printable(scene.active_plate()).filter(|object| {
            !object.summary.is_sound()
                && !self.kept.contains(&object.id)
                && !self.jobs.iter().any(|(job, _)| job.id() == object.id)
        })
    }

    /// Stops offering to repair a model. Its mark stays, and so does its Repair button.
    pub fn keep_as_it_is(&mut self, id: ObjectId) {
        if !self.kept.contains(&id) {
            self.kept.push(id);
        }
    }

    /// Starts repairing one model as `state` asks, unless it is being repaired already.
    ///
    /// The mesh is replaced by the repaired one, so supports, a cavity and a texture
    /// placed on the model before this go with the mesh they were made for.
    pub fn start(&mut self, scene: &Scene, id: ObjectId, state: RepairState, status: &mut Status) {
        let Some(object) = scene.get(id) else {
            return;
        };
        if self.jobs.iter().any(|(job, _)| job.id() == id) {
            return;
        }
        let job = RepairJob::spawn(RepairRequest {
            id,
            name: object.name.clone(),
            mesh: Arc::clone(&object.mesh),
            method: state.method,
            settings: state.settings,
        });
        *status = Status::Info(job.label());
        self.jobs.push((job, Taken::of(object)));
    }

    pub fn is_busy(&self) -> bool {
        !self.jobs.is_empty()
    }

    /// How far the first repair that counts its work has got.
    pub fn counted(&self) -> Option<Counted> {
        self.jobs.iter().find_map(|(job, _)| job.counted())
    }

    /// Gives up on every repair going, which leaves the models as they are.
    pub fn cancel(&mut self, status: &mut Status) {
        if !self.jobs.is_empty() {
            self.jobs.clear();
            *status = Status::Info(tr!("repair-cancelled"));
        }
    }

    /// Puts every repair that has finished into the scene. Returns whether one is still
    /// running, which is what tells the window to keep repainting.
    pub fn poll(&mut self, scene: &mut Scene, status: &mut Status) -> bool {
        let working =
            self.found.poll(scene) | self.shells.poll(scene) | self.hand.poll(scene, status);
        let mut done = Vec::new();
        self.jobs.retain_mut(|(job, taken)| match job.poll() {
            None => true,
            Some(outcome) => {
                done.push((taken.clone(), outcome));
                false
            }
        });

        for (taken, outcome) in done {
            let id = taken.id;
            let replaced = scene.get(id).is_some_and(|object| !taken.is_model(object));
            *status = match outcome {
                RepairOutcome::Done(_) if replaced => {
                    Status::Error(tr!("job-model-changed", name = taken.name))
                }
                RepairOutcome::Done(repaired) => apply(scene, &repaired),
                RepairOutcome::Failed(message) => Status::Error(message),
            };
            // A repair that could not mend it is not offered again; the button still is.
            if scene
                .get(id)
                .is_some_and(|object| !object.summary.is_sound())
            {
                self.keep_as_it_is(id);
            }
        }
        self.is_busy() || working
    }
}

/// Puts a repaired mesh back on the model it came from, and says what repairing it did.
fn apply(scene: &mut Scene, repaired: &Repaired) -> Status {
    let Some(object) = scene.get_mut(repaired.id) else {
        return Status::Error(tr!("repair-gone", name = repaired.name.clone()));
    };
    // A wrap's surface stands whatever the repair under it did.
    if repaired.mended.kept_as_found && !repaired.wrapped {
        return Status::Error(tr!("repair-kept", name = repaired.name.clone()));
    }
    object.reshape(repaired.shaped.clone());
    // What repair did is said in the stage notice; the summary keeps describing the file,
    // so a mended model carries no mark of its own.
    if let Some(orientation) = repaired.mended.orientation {
        object.summary.orientation = orientation;
    }

    let name = &repaired.name;
    let left = repaired.mended.filled.loops_left;
    match (done(repaired), left) {
        (0, 0) => Status::Info(tr!("repair-nothing", name = name.clone())),
        (0, left) => Status::Error(tr!("repair-no-patch", name = name.clone(), count = left)),
        (_, 0) => Status::Info(tr!(
            "repair-done",
            name = name.clone(),
            what = what(repaired).join(", ")
        )),
        (_, left) => Status::Info(tr!(
            "repair-done-open",
            name = name.clone(),
            what = what(repaired).join(", "),
            count = left
        )),
    }
}

/// How many things the repair actually changed, which is what decides whether it has
/// anything to report.
fn done(repaired: &Repaired) -> usize {
    let mended = &repaired.mended;
    mended.filled.loops_filled
        + mended.duplicates_removed
        + mended.stitched.edges_split
        + mended.shells_dropped
        + turned(repaired)
        + usize::from(repaired.method.wraps())
}

/// Faces a Turn repair turned; the other methods turn only the patches they add.
fn turned(repaired: &Repaired) -> usize {
    match (repaired.method, repaired.mended.orientation) {
        (RepairMethod::Mend(MendMethod::Turn), Some(orientation)) => orientation.flipped_faces,
        _ => 0,
    }
}

/// What it changed, in the words the stage notice says it in.
fn what(repaired: &Repaired) -> Vec<String> {
    let mut lines = Vec::new();
    if repaired.mended.filled.loops_filled > 0 {
        lines.push(tr!(
            "repair-closed-holes",
            count = repaired.mended.filled.loops_filled
        ));
    }
    if repaired.mended.stitched.edges_split > 0 {
        lines.push(tr!(
            "repair-stitched-seams",
            count = repaired.mended.stitched.edges_split
        ));
    }
    if repaired.mended.shells_dropped > 0 {
        lines.push(tr!(
            "repair-dropped-shells",
            count = repaired.mended.shells_dropped
        ));
    }
    if repaired.mended.duplicates_removed > 0 {
        lines.push(tr!(
            "repair-dropped-faces",
            count = repaired.mended.duplicates_removed
        ));
    }
    if turned(repaired) > 0 {
        lines.push(tr!("repair-turned-faces", count = turned(repaired)));
    }
    if repaired.method.wraps() {
        lines.push(match (repaired.wrapped, repaired.voxel_mm) {
            (false, _) => tr!("repair-not-wrapped"),
            (true, None) => tr!("repair-wrapped"),
            (true, Some(voxel_mm)) => {
                tr!("repair-wrapped-sampled", lattice = format!("{voxel_mm:.2}"))
            }
        });
    }
    lines
}

/// How many models on the plate being edited will not slice as they stand, which is what
/// the footer reminds the user of before slicing.
pub fn broken_on_the_plate(scene: &Scene) -> usize {
    scene
        .printable(scene.active_plate())
        .filter(|object| !object.summary.is_sound())
        .count()
}

#[cfg(test)]
mod tests;
