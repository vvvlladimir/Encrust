use std::sync::{Arc, Weak};

use core_geometry::{Aabb, Mesh, Scalar, Transform, Vec2, transform_mesh};
use core_plate::{ArrangeSettings, Rect, nearest_free};
use core_volume::{DrainHole, PLUG_SPACING_MM, PlugSource, Plugs, laid_beside, plug_wall_mm};

use super::DrainTool;
use crate::i18n::tr;
use crate::job::{PlugsJob, PlugsRequest, spawn_plugs};
use crate::landing::place;
use crate::scene::{ObjectId, Scene, SceneObject};
use crate::status::Status;

/// What one model's plugs were last laid for, in its own space: they are laid again
/// whenever it changes (ADR 0313). The default is laid for nothing, a model's first word.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlugsLaid {
    holes: Vec<DrainHole>,
    wall_mm: Option<Scalar>,
    gap_mm: Scalar,
}

/// The plugs being cut for one model, and what they are cut for.
#[derive(Debug)]
pub struct Laying {
    model: ObjectId,
    /// The mesh they are cut from, not held so a model replaced meanwhile is let go.
    mesh: Weak<Mesh>,
    wanted: PlugsLaid,
    /// The model asks for other plugs now: these are thrown away when they land, and the
    /// next cut starts then rather than beside this one, which cannot be stopped.
    stale: bool,
    /// The number of the edit that asked for them, `History::next_edit` when they started.
    asked_by: u64,
    job: PlugsJob,
}

/// Plugs a model asks for that cannot be started yet, and the edit that first asked for them.
#[derive(Debug)]
pub struct Waiting {
    model: ObjectId,
    wanted: PlugsLaid,
    asked_by: u64,
}

/// A cut that has ended, waiting to be laid into the edit that asked for it.
#[derive(Debug)]
pub struct Ended {
    model: ObjectId,
    wanted: PlugsLaid,
    asked_by: u64,
    made: Result<Plugs, String>,
}

impl Ended {
    /// The number of the edit that asked for these plugs, for `History::land`.
    pub fn asked_by(&self) -> u64 {
        self.asked_by
    }
}

/// The piece of a model read round its last holes, kept for the next cut while the model is
/// the same mesh.
#[derive(Debug)]
pub struct ReadModel {
    model: ObjectId,
    mesh: Weak<Mesh>,
    source: PlugSource,
}

impl DrainTool {
    /// Starts cutting again the plugs of every model whose holes, wall or plug gap moved
    /// since they were last laid; they land on a later frame through [`Self::land_plugs`].
    /// `drift_mm` is what the resin leaves a fit wider by (`Compensation::fit_drift_mm`). A
    /// model read back from a project has no record and takes its plugs as they stand, the
    /// ones deleted before it was saved too. No cut starts while `settled` is false, in the
    /// middle of a drag: the plugs follow once it ends. `asked_by` is `History::next_edit`.
    pub fn lay_plugs(&mut self, scene: &mut Scene, drift_mm: Scalar, settled: bool, asked_by: u64) {
        let due: Vec<(ObjectId, PlugsLaid, bool)> = scene
            .objects()
            .iter()
            .filter(|object| object.plug_of.is_none())
            .filter_map(|object| {
                let wanted = self.plugs_for(object, drift_mm);
                let kept = match &object.plugs_laid {
                    Some(laid) if *laid == wanted => return None,
                    Some(_) => false,
                    None => true,
                };
                Some((object.id, wanted, kept))
            })
            .collect();
        // A cut for what a model no longer asks for, as after an undo, or from a mesh since
        // replaced, is not laid.
        for laying in &mut self.laying {
            let same_mesh = scene
                .get(laying.model)
                .is_some_and(|object| laying.mesh.as_ptr() == Arc::as_ptr(&object.mesh));
            laying.stale = !same_mesh
                || !due
                    .iter()
                    .any(|(id, wanted, _)| *id == laying.model && *wanted == laying.wanted);
        }

        self.waiting.retain(|waiting| {
            due.iter()
                .any(|(id, wanted, _)| *id == waiting.model && *wanted == waiting.wanted)
        });

        for (id, wanted, kept) in due {
            if kept || wanted.holes.is_empty() {
                if !kept {
                    take_away(scene, id);
                }
                if let Some(object) = scene.get_mut(id) {
                    object.plugs_laid = Some(wanted);
                }
            } else {
                self.ask(scene, id, wanted, settled, asked_by);
            }
        }
    }

    /// Starts cutting the plugs `wanted` for model `id` where nothing holds them back, else
    /// keeps them waiting, under the edit that first asked for them either way.
    fn ask(
        &mut self,
        scene: &Scene,
        id: ObjectId,
        wanted: PlugsLaid,
        settled: bool,
        asked_by: u64,
    ) {
        let asked_by = self
            .waiting
            .iter()
            .find(|waiting| waiting.model == id)
            .map_or(asked_by, |waiting| waiting.asked_by);
        self.waiting.retain(|waiting| waiting.model != id);
        if settled && !self.laying.iter().any(|laying| laying.model == id) {
            self.start_plugs(scene, id, wanted, asked_by);
        } else {
            self.waiting.push(Waiting {
                model: id,
                wanted,
                asked_by,
            });
        }
    }

    /// Starts cutting the plugs `wanted` for model `id`, handing the cut the model as last
    /// read where it is still the same mesh.
    fn start_plugs(&mut self, scene: &Scene, id: ObjectId, wanted: PlugsLaid, asked_by: u64) {
        let Some(object) = scene.get(id) else {
            return;
        };
        let source = self
            .read
            .take_if(|read| read.model == id && read.mesh.as_ptr() == Arc::as_ptr(&object.mesh))
            .map(|read| read.source);
        let job = spawn_plugs(PlugsRequest {
            mesh: Arc::clone(&object.mesh),
            bvh: Arc::clone(&object.bvh),
            source,
            holes: wanted.holes.clone(),
            wall_mm: wanted.wall_mm,
            gap_mm: wanted.gap_mm,
        });
        self.laying.push(Laying {
            model: id,
            mesh: Arc::downgrade(&object.mesh),
            wanted,
            stale: false,
            asked_by,
            job,
        });
    }

    /// The cuts that have ended and still stand for what their model asks, keeping the model
    /// each read for the next cut. Cheap while none has ended, so it runs every frame.
    pub fn poll_plugs(&mut self) -> Vec<Ended> {
        let mut ended = Vec::new();
        let mut index = 0;
        while index < self.laying.len() {
            let Some((source, made)) = self.laying[index].job.poll() else {
                index += 1;
                continue;
            };
            let laying = self.laying.swap_remove(index);
            if let Some(source) = source.filter(|_| laying.mesh.strong_count() > 0) {
                self.read = Some(ReadModel {
                    model: laying.model,
                    mesh: laying.mesh,
                    source,
                });
            }
            if !laying.stale {
                ended.push(Ended {
                    model: laying.model,
                    wanted: laying.wanted,
                    asked_by: laying.asked_by,
                    made,
                });
            }
        }
        ended
    }

    /// Lays the plugs `ended` cut in place of the ones laid before, on a plate `plate_mm`
    /// across, and says so where a plug is only sampled, missing or off the free plate.
    pub fn land_plugs(scene: &mut Scene, ended: &Ended, plate_mm: Vec2, status: &mut Status) {
        let id = ended.model;
        match &ended.made {
            Ok(made) => {
                let crowded = replace(scene, id, &ended.wanted.holes, &made.meshes, plate_mm);
                let notes: Vec<String> = [
                    (
                        made.sampled,
                        tr!("drain-plugs-sampled", count = made.sampled),
                    ),
                    (made.closed, tr!("drain-plugs-closed", count = made.closed)),
                    (crowded, tr!("drain-plugs-crowded", count = crowded)),
                ]
                .into_iter()
                .filter(|(count, _)| *count > 0)
                .map(|(_, note)| note)
                .collect();
                if let Some((first, rest)) = notes.split_first() {
                    *status = Status::Noted(first.clone(), rest.to_vec());
                }
            }
            Err(message) => {
                let error = anyhow::Error::msg(message.clone()).context(tr!("drain-plugs-failed"));
                *status = Status::failed(&error);
            }
        }
        if let Some(object) = scene.get_mut(id) {
            object.plugs_laid = Some(ended.wanted.clone());
        }
    }

    /// Whether plugs are being cut.
    pub fn laying_plugs(&self) -> bool {
        !self.laying.is_empty()
    }

    /// Lets go of the piece of a model kept read for its plugs once they are no longer being
    /// worked on.
    pub fn forget_read_model(&mut self) {
        self.read = None;
    }

    /// The plugs `object` asks for now: one a hole as it is cut, through the wall.
    fn plugs_for(&self, object: &SceneObject, drift_mm: Scalar) -> PlugsLaid {
        let least = object.transform.least_scale();
        PlugsLaid {
            holes: object.hollow.cut().0.to_vec(),
            wall_mm: object
                .hollow
                .asked()
                .and_then(|asked| plug_wall_mm(asked.mode, asked.thickness_mm / least)),
            gap_mm: (self.state_for(object).plug_gap_mm - drift_mm) / least,
        }
    }
}

/// Takes away the plugs laid for model `id`, leaving the selection where it was.
fn take_away(scene: &mut Scene, id: ObjectId) {
    let selection = scene.selection().to_vec();
    let old: Vec<ObjectId> = scene
        .objects()
        .iter()
        .filter(|object| object.plug_of == Some(id))
        .map(|object| object.id)
        .collect();
    for plug in old {
        scene.remove(plug);
    }
    scene.select_many(&selection);
}

/// Takes away the plugs laid for model `id` and lays `made`, cut for its `holes`, on a plate
/// `plate_mm` across, leaving the selection where it was. Answers how many found no room.
fn replace(
    scene: &mut Scene,
    id: ObjectId,
    holes: &[DrainHole],
    made: &[Mesh],
    plate_mm: Vec2,
) -> usize {
    take_away(scene, id);
    let selection = scene.selection().to_vec();
    let found = scene.get(id).and_then(|object| {
        let beside = object.world_bounds()?;
        Some((beside, object.transform, object.name.clone(), object.plate))
    });
    let mut crowded = 0;
    if let Some((beside, transform, name, plate)) = found {
        // `laid_beside` leaves out an empty plug, so each is numbered after its own hole.
        let numbers = (1..).zip(made).filter(|(_, plug)| !plug.is_empty());
        let laid = numbers
            .map(|(number, _)| number)
            .zip(laid_beside(made, holes, transform, beside));
        crowded = lay_out(scene, id, plate, &name, laid, plate_mm);
    }
    scene.select_many(&selection);
    crowded
}

/// Puts `plugs`, numbered and already in plate coordinates, on `plate`, each marked as a plug
/// of `model` and moved to the nearest spot clear of what stands there, inside the plate
/// where it fits. Answers how many fit nowhere and were left where they were laid.
fn lay_out(
    scene: &mut Scene,
    model: ObjectId,
    plate: u32,
    name: &str,
    plugs: impl Iterator<Item = (usize, Mesh)>,
    plate_mm: Vec2,
) -> usize {
    let mut taken = covered(scene, plate);
    let settings = ArrangeSettings {
        clearance_mm: PLUG_SPACING_MM,
        ..ArrangeSettings::default()
    };
    let mut crowded = 0;
    for (number, plug) in plugs {
        let Some(bounds) = plug.aabb() else {
            continue;
        };
        let (min, max) = (bounds.mins.truncate(), bounds.maxs.truncate());
        let toward = (min + max) / 2.0;
        let at =
            nearest_free(max - min, &taken, toward, plate_mm, &settings).unwrap_or_else(|| {
                crowded += 1;
                toward
            });
        let shift = (at - toward).extend(0.0);
        taken.push(Rect {
            min: min + shift.truncate(),
            max: max + shift.truncate(),
        });
        let plug = transform_mesh(&plug, Transform::from_translation(shift));
        let name = tr!("drain-plug-name", name = name.to_owned(), index = number);
        let (id, _) = place(scene, plate, name, plug);
        if let Some(plug) = scene.get_mut(id) {
            plug.plug_of = Some(model);
        }
    }
    crowded
}

/// What the models on `plate`, their supports and their rafts cover across it.
fn covered(scene: &Scene, plate: u32) -> Vec<Rect> {
    let rect = |bounds: Aabb| Rect {
        min: bounds.mins.truncate(),
        max: bounds.maxs.truncate(),
    };
    scene
        .printable(plate)
        .flat_map(|object| {
            let supports = object
                .supports
                .drawn()
                .iter()
                .filter_map(|group| group.mesh.aabb());
            object.world_bounds().into_iter().chain(supports)
        })
        .map(rect)
        .collect()
}

#[cfg(test)]
mod tests;
