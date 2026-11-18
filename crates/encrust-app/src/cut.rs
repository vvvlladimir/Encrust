use std::sync::Arc;

use anyhow::{Result, anyhow};
use core_document::{CutAsked, CutModel, Role, axis_normal};
use core_geometry::{Plane, Scalar, Vec3, transform_mesh};
use core_joint::{JointKind, Joints};
use core_volume::{PartSettings, PartedOff};

pub use core_engine::project::Keep;

use core_engine::project::{Axis, CutState};

use crate::i18n::tr;
use crate::job::{
    Counted, CutJob, CutOutcome, CutRequest, SplitJob, SplitOutcome, SplitRequest, spawn_cut,
    spawn_split,
};
use crate::landing::{self, mending, place_all, place_piece};
use crate::scene::{ObjectId, Scene, SceneObject, Taken};
use crate::status::Status;

/// Where the Cut tool's plane is, and what it does with the halves.
#[derive(Debug)]
pub struct CutTool {
    /// What the panel is set to, in the shape a project writes it down in (ADR 0191),
    /// joints included (ADR 0306). Its `height_mm` is where the plane crosses the axis:
    /// across Z the height above the plate, across X or Y the distance from the model's
    /// centre of mass.
    pub state: CutState,
    /// Whether each half is turned to stand on the face the cut left. The window's own,
    /// like a view switch: a project does not write it down.
    pub lay_flat: bool,
    /// The model the plane was last put through the middle of, so picking another one
    /// moves the plane to it.
    pub centred_on: Option<ObjectId>,
    running: Option<Running>,
    /// A split under way, and the model it splits.
    splitting: Option<(SplitJob, Taken)>,
    /// Whether the next click on a model marks the part Split cuts off it.
    pub picking_part: bool,
    /// The model a part is marked on, and the point, in the model's own coordinates so
    /// that it moves with it. A mark on a mesh that has since been replaced marks nothing.
    part_mark: Option<(Taken, Vec3)>,
}

/// A cut under way, with what the tool was set to when it started.
#[derive(Debug)]
struct Running {
    job: CutJob,
    taken: Taken,
    asked: CutAsked,
}

impl Default for CutTool {
    fn default() -> Self {
        Self {
            state: CutState {
                axis: Axis::Z,
                height_mm: 10.0,
                keep: Keep::Both,
                joints: Joints::default(),
                parts: core_volume::PartSettings::default(),
            },
            lay_flat: false,
            centred_on: None,
            running: None,
            splitting: None,
            picking_part: false,
            part_mark: None,
        }
    }
}

impl CutTool {
    /// The plane the tool is set to for one model, in plate coordinates.
    pub fn plane(&self, object: &SceneObject) -> Option<Plane> {
        Plane::at(axis_normal(self.state.axis), self.position_mm(object))
    }

    /// Where the plane crosses its axis for one model, in plate millimetres.
    pub fn position_mm(&self, object: &SceneObject) -> Scalar {
        match self.state.axis {
            Axis::X => object.pivot().translation.x + self.state.height_mm,
            Axis::Y => object.pivot().translation.y + self.state.height_mm,
            Axis::Z => self.state.height_mm,
        }
    }

    /// Turns the plane to `axis`, through the model's centre of mass across X or Y and
    /// halfway up it across Z: the offset meant for one axis means nothing on another.
    pub fn set_axis(&mut self, axis: Axis, object: &SceneObject) {
        self.state.axis = axis;
        self.state.height_mm = match axis {
            Axis::X | Axis::Y => 0.0,
            Axis::Z => object
                .world_bounds()
                .map_or(object.pivot().translation.z, |bounds| {
                    (bounds.mins.z + bounds.maxs.z) / 2.0
                }),
        };
    }

    /// Puts the plane through the middle of `id` on the axis it is set to, unless it was
    /// already put through this model.
    pub fn centre_on(&mut self, id: ObjectId, object: &SceneObject) {
        if self.centred_on != Some(id) {
            self.centred_on = Some(id);
            self.set_axis(self.state.axis, object);
        }
    }

    /// Starts cutting one model in two off the drawing thread; [`CutTool::poll`] puts the
    /// halves in its place: new geometry, its holes cut into it, with no supports.
    pub fn start(&mut self, scene: &Scene, id: ObjectId) -> Result<()> {
        let object = scene
            .get(id)
            .ok_or_else(|| anyhow!(tr!("cut-nothing-selected")))?;
        if self.plane(object).is_none() {
            return Err(anyhow!(tr!("cut-no-direction")));
        }
        let asked = CutAsked {
            axis: self.state.axis,
            at_mm: self.position_mm(object),
            keep: self.state.keep,
            joints: self.state.joints,
            lay_flat: self.lay_flat,
        };
        self.start_asked(scene, id, asked)
    }

    /// Starts the cut `asked` describes on one model, as [`CutTool::start`] does, whatever the
    /// panel is set to: what an edit handed over the window link asks (ADR 0466).
    pub fn start_asked(&mut self, scene: &Scene, id: ObjectId, asked: CutAsked) -> Result<()> {
        let object = scene
            .get(id)
            .ok_or_else(|| anyhow!(tr!("cut-nothing-selected")))?;
        let uncut = object.uncut();
        let drilled = object.hollow.drilled(uncut).map(|drilled| &drilled.mesh);
        let job = spawn_cut(CutRequest {
            id,
            model: CutModel {
                printed: Arc::clone(drilled.unwrap_or(uncut)),
                cuts: drilled
                    .is_none()
                    .then(|| object.hollow.cut_bodies().cloned())
                    .flatten(),
                solid: object.hollow.shell().map(|_| Arc::clone(&object.mesh)),
                transform: object.transform,
            },
            asked,
        });
        self.running = Some(Running {
            job,
            taken: Taken::of(object),
            asked,
        });
        Ok(())
    }

    /// Lands a finished cut in the scene. Returns whether one is still going, which is
    /// what tells the window to keep repainting.
    pub fn poll(&mut self, scene: &mut Scene, status: &mut Status) -> bool {
        if let Some(outcome) = self.running.as_mut().and_then(|running| running.job.poll())
            && let Some(running) = self.running.take()
        {
            *status = running.land(scene, outcome);
        }
        if let Some(outcome) = self.splitting.as_mut().and_then(|(job, _)| job.poll())
            && let Some((_, taken)) = self.splitting.take()
        {
            if matches!(outcome, SplitOutcome::PartedOff(_)) {
                self.part_mark = None;
            }
            *status = land_parts(scene, &taken, outcome);
        }
        self.is_busy()
    }

    pub fn is_busy(&self) -> bool {
        self.running.is_some() || self.splitting.is_some()
    }

    /// How far the split going has got, while it has counted something; a cut counts
    /// nothing.
    pub fn counted(&self) -> Option<Counted> {
        self.splitting.as_ref()?.0.counted()
    }

    /// Gives up on the cut or split going, which leaves the model as it is.
    pub fn cancel(&mut self, status: &mut Status) {
        if self.running.take().is_some() | self.splitting.take().is_some() {
            *status = Status::Info(tr!("cut-cancelled"));
        }
    }

    /// Marks the part at `point` on the plate as the one Split cuts off `object`, and
    /// ends the picking.
    pub fn mark_part(&mut self, object: &SceneObject, point: Vec3) {
        let local = object
            .transform
            .to_matrix()
            .inverse()
            .transform_point3(point);
        self.part_mark = Some((Taken::of(object), local));
        self.picking_part = false;
    }

    /// Where the part is marked on `object`, in plate coordinates, if it is.
    pub fn marked_on(&self, object: &SceneObject) -> Option<Vec3> {
        let (marked, local) = self.part_mark.as_ref()?;
        marked
            .is_model(object)
            .then(|| object.transform.to_matrix().transform_point3(*local))
    }

    /// Forgets the part marked, as the toggle that marked it does.
    pub fn unmark_part(&mut self) {
        self.part_mark = None;
    }

    /// Starts breaking one model into the parts it is made of, or cutting off the part
    /// marked on it, off the drawing thread, as the tool's `parts` settings say;
    /// [`CutTool::poll`] puts them on the plate.
    pub fn start_split(&mut self, scene: &Scene, id: ObjectId) -> Result<()> {
        let at = (scene.get(id)).and_then(|object| self.marked_on(object));
        self.start_split_asked(scene, id, self.state.parts, at)
    }

    /// Starts the split of one model into `settings`' parts, or the cutting off of the part at
    /// plate point `at`, whatever the panel is set to: what an edit handed over the window
    /// link asks (ADR 0466).
    pub fn start_split_asked(
        &mut self,
        scene: &Scene,
        id: ObjectId,
        settings: PartSettings,
        at: Option<Vec3>,
    ) -> Result<()> {
        let object = scene
            .get(id)
            .ok_or_else(|| anyhow!(tr!("cut-nothing-to-split")))?;
        let mesh = object.printed();
        let job = spawn_split(SplitRequest {
            id,
            placed: transform_mesh(mesh, object.transform),
            settings,
            at,
        });
        self.splitting = Some((job, Taken::of(object)));
        Ok(())
    }
}

impl Running {
    fn land(self, scene: &mut Scene, outcome: CutOutcome) -> Status {
        let name = self.taken.name.clone();
        let (pieces, placed, bare, open_loops, unjoined) = match outcome {
            CutOutcome::Cut {
                pieces,
                placed,
                bare,
                open_loops,
                unjoined,
            } => (pieces, placed, bare, open_loops, unjoined),
            CutOutcome::Missed => return Status::Error(tr!("cut-misses", name = name)),
            CutOutcome::Undrilled => return Status::Error(tr!("cut-undrilled", name = name)),
            CutOutcome::Failed(message) => return Status::Error(message),
        };
        if let Err(changed) = landing::take_off(scene, std::slice::from_ref(&self.taken)) {
            return Status::Error(changed);
        }
        let mended = pieces.iter().filter(|piece| piece.mended).count();
        for piece in pieces {
            let piece_name = piece_name(&name, piece.role);
            place_piece(scene, self.taken.plate, piece_name, piece);
        }
        if open_loops > 0 {
            let said = tr!("cut-done-open", name = name, open = open_loops);
            return Status::noted(said, mending(mended).into_iter().collect());
        }
        let kind = (self.asked.joints.kind).filter(|_| self.asked.keep == Keep::Both);
        let mut notes = joint_notes(kind, placed, bare, unjoined);
        notes.extend(mending(mended));
        let said = tr!(
            "cut-done",
            name = name,
            axis = self.asked.axis.label(),
            position = format!("{:.1}", self.asked.at_mm)
        );
        Status::noted(said, notes)
    }
}

/// A piece's name in the window's language, after the model it was cut from.
fn piece_name(name: &str, role: Role) -> String {
    let name = name.to_owned();
    match role {
        Role::Kept | Role::Copy => name,
        Role::Lower => tr!("cut-lower-name", name = name),
        Role::Upper => tr!("cut-upper-name", name = name),
        Role::Dowel(index) => tr!("cut-dowel-name", name = name, index = index),
        Role::Part(index) => tr!("cut-part-name", name = name, index = index),
    }
}

/// What the stage notice notes of the joint, and of the pieces of the face it missed.
fn joint_notes(
    kind: Option<JointKind>,
    placed: usize,
    bare: usize,
    unjoined: Option<String>,
) -> Vec<String> {
    let Some(kind) = kind else {
        return Vec::new();
    };
    if let Some(reason) = unjoined {
        return vec![tr!("cut-joint-failed", reason = reason)];
    }
    let joint = match (kind, placed) {
        (_, 0) => return vec![tr!("cut-no-room-for-joint")],
        (JointKind::Pins, count) => tr!("cut-with-pins", count = count),
        (JointKind::Dowels, count) => tr!("cut-with-dowels", count = count),
        (JointKind::Magnets, count) => tr!("cut-with-magnets", count = count),
        (JointKind::Key, count) => tr!("cut-with-key", count = count),
    };
    let bare = (bare > 0).then(|| tr!("cut-bare", count = bare));
    std::iter::once(joint).chain(bare).collect()
}

/// Puts the parts a split found on the plate in place of the model it took. A model that
/// is one part and lost no scrap is left alone, supports and all.
fn land_parts(scene: &mut Scene, taken: &Taken, outcome: SplitOutcome) -> Status {
    let name = &taken.name;
    let parts = match outcome {
        SplitOutcome::Split(parts) => parts,
        SplitOutcome::PartedOff(parted) => return land_parted_off(scene, taken, parted),
        SplitOutcome::Failed(message) => return Status::Error(message),
    };
    let count = parts.parts.len();
    if count < 2 && parts.joined + parts.dropped == 0 {
        return Status::Info(tr!("cut-one-piece", name = name.clone()));
    }
    if let Err(changed) = landing::take_off(scene, std::slice::from_ref(taken)) {
        return Status::Error(changed);
    }
    let mut notes = scrap_notes(parts.joined, parts.dropped);
    let named = parts.parts.into_iter().enumerate().map(|(index, part)| {
        let part_name = match count {
            1 => name.clone(),
            _ => tr!("cut-part-name", name = name.clone(), index = index + 1),
        };
        (part_name, part)
    });
    notes.extend(mending(place_all(scene, taken.plate, named)));
    let said = match count {
        0 | 1 => tr!("cut-cleaned", name = name.clone()),
        count => tr!("cut-split-done", name = name.clone(), count = count),
    };
    Status::noted(said, notes)
}

/// Puts the marked part and the rest of the model on the plate in its place.
fn land_parted_off(scene: &mut Scene, taken: &Taken, parted: PartedOff) -> Status {
    if let Err(changed) = landing::take_off(scene, std::slice::from_ref(taken)) {
        return Status::Error(changed);
    }
    let name = &taken.name;
    let sides = [parted.part, parted.rest]
        .into_iter()
        .enumerate()
        .map(|(index, side)| {
            let side_name = tr!("cut-part-name", name = name.clone(), index = index + 1);
            (side_name, side)
        });
    let mended = place_all(scene, taken.plate, sides);
    let said = tr!("cut-parted-off", name = name.clone());
    Status::noted(said, mending(mended).into_iter().collect())
}

/// What the stage notice notes of the scraps a split joined to their parts or dropped.
fn scrap_notes(joined: usize, dropped: usize) -> Vec<String> {
    let joined = (joined > 0).then(|| tr!("cut-split-joined", count = joined));
    let dropped = (dropped > 0).then(|| tr!("cut-split-dropped", count = dropped));
    joined.into_iter().chain(dropped).collect()
}

#[cfg(test)]
mod tests;
