use std::sync::Arc;

use anyhow::{Result, anyhow};
use core_geometry::{Mesh, Plane, Scalar, Transform, Vec3, transform_mesh};
use core_joint::{Dowel, JointKind, Joints};
use core_volume::PartedOff;

pub use core_engine::project::Keep;

use core_engine::project::{Axis, CutState};

use crate::i18n::tr;
use crate::job::{
    Counted, CutJob, CutOutcome, CutRequest, SplitJob, SplitOutcome, SplitRequest, spawn_cut,
    spawn_split,
};
use crate::landing::{self, mending, place, place_all};
use crate::scene::{ObjectId, Scene, SceneObject, Taken};
use crate::status::Status;

/// Space left between a cut's lower half and the dowels laid out beside it, and between
/// one dowel and the next, millimetres.
const DOWEL_GAP_MM: Scalar = 3.0;

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
    axis: Axis,
    position_mm: Scalar,
    up: Vec3,
    keep: Keep,
    joints: Joints,
    lay_flat: bool,
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
        Plane::at(normal(self.state.axis), self.position_mm(object))
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
        let plane = self
            .plane(object)
            .ok_or_else(|| anyhow!(tr!("cut-no-direction")))?;
        let uncut = object.uncut();
        let drilled = object.hollow.drilled(uncut).map(|drilled| &drilled.mesh);
        let job = spawn_cut(CutRequest {
            printed: Arc::clone(drilled.unwrap_or(uncut)),
            cuts: drilled
                .is_none()
                .then(|| object.hollow.cut_bodies().cloned())
                .flatten(),
            solid: object.hollow.shell().map(|_| Arc::clone(&object.mesh)),
            transform: object.transform,
            plane,
            keep: self.state.keep,
            joints: self.state.joints,
        });
        self.running = Some(Running {
            job,
            taken: Taken::of(object),
            axis: self.state.axis,
            position_mm: self.position_mm(object),
            up: plane.normal,
            keep: self.state.keep,
            joints: self.state.joints,
            lay_flat: self.lay_flat,
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
        let object = scene
            .get(id)
            .ok_or_else(|| anyhow!(tr!("cut-nothing-to-split")))?;
        let mesh = object.printed();
        let job = spawn_split(SplitRequest {
            placed: transform_mesh(mesh, object.transform),
            settings: self.state.parts,
            at: self.marked_on(object),
        });
        self.splitting = Some((job, Taken::of(object)));
        Ok(())
    }
}

impl Running {
    fn land(self, scene: &mut Scene, outcome: CutOutcome) -> Status {
        let name = self.taken.name.clone();
        let (joined, open_loops, unjoined) = match outcome {
            CutOutcome::Cut {
                joined,
                open_loops,
                unjoined,
            } => (joined, open_loops, unjoined),
            CutOutcome::Missed => return Status::Error(tr!("cut-misses", name = name)),
            CutOutcome::Undrilled => return Status::Error(tr!("cut-undrilled", name = name)),
            CutOutcome::Failed(message) => return Status::Error(message),
        };
        if let Err(changed) = landing::take_off(scene, std::slice::from_ref(&self.taken)) {
            return Status::Error(changed);
        }
        let kind = self.joints.kind.filter(|_| self.keep == Keep::Both);
        // Pins and a tongue stand on the lower half's cut, which it then cannot lie on.
        let raised = matches!(kind, Some(JointKind::Pins | JointKind::Key)) && joined.placed > 0;
        let (mended, lower) = self.put_down(scene, joined.below, joined.above, raised);
        if let Some(lower) = lower {
            lay_out_dowels(scene, lower, &name, &joined.loose);
        }
        if open_loops > 0 {
            let said = tr!("cut-done-open", name = name, open = open_loops);
            return Status::noted(said, mending(mended).into_iter().collect());
        }
        let mut notes = joint_notes(kind, joined.placed, joined.bare, unjoined);
        notes.extend(mending(mended));
        let said = tr!(
            "cut-done",
            name = name,
            axis = self.axis.label(),
            position = format!("{:.1}", self.position_mm)
        );
        Status::noted(said, notes)
    }

    /// Puts the halves kept on the plate, each laid on its cut face if asked and unless
    /// it is the lower one and `raised`. Answers how many had to be mended, and the lower.
    fn put_down(
        &self,
        scene: &mut Scene,
        below: Mesh,
        above: Mesh,
        raised: bool,
    ) -> (usize, Option<ObjectId>) {
        let name = &self.taken.name;
        let up = self.up;
        let pieces = match self.keep {
            Keep::Both => vec![
                (tr!("cut-lower-name", name = name.clone()), below, up),
                (tr!("cut-upper-name", name = name.clone()), above, -up),
            ],
            Keep::Below => vec![(name.clone(), below, up)],
            Keep::Above => vec![(name.clone(), above, -up)],
        };
        let (mut mended, mut lower) = (0, None);
        for (name, mesh, cut_face) in pieces {
            let (piece, was_mended) = place(scene, self.taken.plate, name, mesh);
            mended += usize::from(was_mended);
            let is_lower = cut_face == up;
            if is_lower {
                lower = Some(piece);
            }
            if let Some(object) = scene.get_mut(piece)
                && self.lay_flat
                && !(is_lower && raised)
            {
                object.lay_face_down(cut_face);
            }
        }
        (mended, lower)
    }
}

/// Stands each dowel on the plate in a row beside the lower half `beside`.
fn lay_out_dowels(scene: &mut Scene, beside: ObjectId, name: &str, dowels: &[Dowel]) {
    let Some((bounds, plate)) = scene
        .get(beside)
        .and_then(|object| Some((object.world_bounds()?, object.plate)))
    else {
        return;
    };
    let mut x = bounds.maxs.x + DOWEL_GAP_MM;
    for (index, dowel) in dowels.iter().enumerate() {
        let radius = dowel.radius_mm;
        let offset = Vec3::new(x + radius, bounds.mins.y + radius, 0.0);
        let placed = transform_mesh(&dowel.mesh, Transform::from_translation(offset));
        let name = tr!("cut-dowel-name", name = name.to_owned(), index = index + 1);
        place(scene, plate, name, placed);
        x += 2.0 * radius + DOWEL_GAP_MM;
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

fn normal(axis: Axis) -> Vec3 {
    match axis {
        Axis::X => Vec3::X,
        Axis::Y => Vec3::Y,
        Axis::Z => Vec3::Z,
    }
}

#[cfg(test)]
mod tests;
