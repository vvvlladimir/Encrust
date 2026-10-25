//! What the window does about a model that arrived broken: the offer at the foot of the
//! stage, and the runs that close the holes.
//!
//! Welding, orientation and dropping the faces laid over others are done on import without
//! asking, because none of them changes what the model encloses (ADR 0005, 0270). Closing a
//! hole invents surface that was never in the file, so it is offered without blocking
//! anything and waits for the Repair button; see
//! `docs/decisions/0270-repair-cancels-what-encloses-nothing-and-never-worsens.md`.

use std::sync::Arc;

use crate::i18n::tr;
use crate::job::{RepairJob, RepairOutcome, RepairRequest, Repaired};
use crate::scene::{ObjectId, Scene, SceneObject, Taken};
use crate::status::Status;

/// The repairs under way, and the broken models the user chose to keep as they came.
#[derive(Debug, Default)]
pub struct Repairs {
    /// Each with the model it took, so that a mesh replaced since is not mended over.
    jobs: Vec<(RepairJob, Taken)>,
    kept: Vec<ObjectId>,
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

    /// Starts closing the holes in one model.
    ///
    /// The mesh is replaced by the repaired one, so supports, a cavity and a texture
    /// placed on the model before this go with the mesh they were made for.
    pub fn start(&mut self, scene: &Scene, id: ObjectId, status: &mut Status) {
        let Some(object) = scene.get(id) else {
            return;
        };
        let job = RepairJob::spawn(RepairRequest {
            id,
            name: object.name.clone(),
            mesh: Arc::clone(&object.mesh),
        });
        *status = Status::Info(job.label());
        self.jobs.push((job, Taken::of(object)));
    }

    pub fn is_busy(&self) -> bool {
        !self.jobs.is_empty()
    }

    /// Puts every repair that has finished into the scene. Returns whether one is still
    /// running, which is what tells the window to keep repainting.
    pub fn poll(&mut self, scene: &mut Scene, status: &mut Status) -> bool {
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
        self.is_busy()
    }
}

/// Puts a repaired mesh back on the model it came from, and says what repairing it did.
fn apply(scene: &mut Scene, repaired: &Repaired) -> Status {
    let Some(object) = scene.get_mut(repaired.id) else {
        return Status::Error(tr!("repair-gone", name = repaired.name.clone()));
    };
    if repaired.mended.kept_as_found {
        return Status::Error(tr!("repair-kept", name = repaired.name.clone()));
    }
    object.reshape(repaired.shaped.clone());
    // What repair did is said in the stage notice; the summary keeps describing the file,
    // so a mended model carries no mark of its own.
    object.summary.orientation = repaired.mended.orientation;

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
    repaired.mended.filled.loops_filled + repaired.mended.duplicates_removed
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
    if repaired.mended.duplicates_removed > 0 {
        lines.push(tr!(
            "repair-dropped-faces",
            count = repaired.mended.duplicates_removed
        ));
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
