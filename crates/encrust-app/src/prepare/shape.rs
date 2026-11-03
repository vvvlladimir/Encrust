//! The modelling steps of a run — faces changed by hand, a relief, combine, a label, a cut, a
//! split, the plate arranged — each done by the window's own tool, set as the plan says and
//! started on one model at a time, as the command line does them before it prepares one.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use core_engine::project::{Axis, Keep};
use core_geometry::{PlacedHit, Region, Scalar, Vec3};
use core_joint::JointKind;
use core_text::{LABEL_DEPTH_MM, LETTER_HEIGHT_MM};
use core_volume::Boolean;

use super::Step;
use crate::files::Handed;
use crate::job::{Edit, EditJob, EditOutcome};
use crate::scene::{ObjectId, Scene, Taken};
use crate::state::{Doc, Machine, Tools};

/// What the modelling steps of a plan do; each `None` leaves its step out.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Shaping {
    pub mend: Option<Mend>,
    pub relief: Option<ReliefAsk>,
    /// Every model of the run joined into the first, as `--combine`.
    pub combine: Option<Boolean>,
    pub label: Option<LabelAsk>,
    pub cut: Option<CutAsk>,
    pub split: Option<SplitAsk>,
    pub arrange: bool,
}

/// Faces and shells changed by hand, numbered as the command line numbers them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mend {
    pub flip_faces: Vec<u32>,
    pub bridges: Vec<([u32; 2], [u32; 2])>,
    pub drop_faces: Vec<u32>,
    /// Counted from one, as `inspect` lists them.
    pub drop_shells: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReliefAsk {
    pub depth_mm: Scalar,
    /// A picture tiled over the model, or the model's own texture where `None`.
    pub image: Option<PathBuf>,
    pub tile_mm: Option<Scalar>,
    pub tile_angle_deg: Option<Scalar>,
    pub invert: bool,
    pub symmetric: bool,
    pub smooth_px: Option<Scalar>,
    pub hold_bottom_deg: Option<Scalar>,
    pub hold_top_deg: Option<Scalar>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LabelAsk {
    pub text: String,
    /// The plate point it is written nearest to.
    pub at_mm: Vec3,
    pub height_mm: Option<Scalar>,
    pub depth_mm: Option<Scalar>,
    pub sunk: bool,
    pub follow: bool,
    pub angle_deg: Option<Scalar>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CutAsk {
    pub axis: Axis,
    /// The plate coordinate along the axis, millimetres.
    pub at_mm: Scalar,
    pub keep: Keep,
    pub joint: Option<JointKind>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SplitAsk {
    /// Into its bodies, a piece holding less than this many mm³ joined back.
    Parts { smallest_mm3: Scalar },
    /// The part this plate point is on cut off.
    PartOff { at_mm: Vec3 },
}

/// A modelling step going: the tool started on one model at a time, the ones left, and the
/// plate as it stood before, to tell the pieces the step made.
#[derive(Debug)]
pub(super) struct Shaped {
    step: Step,
    shaping: Shaping,
    left: VecDeque<ObjectId>,
    mending: Option<(EditJob, Taken, VecDeque<Edit>)>,
    before: Vec<ObjectId>,
}

impl Shaping {
    /// Whether `step` is one of these and is asked for.
    pub(super) fn wants(&self, step: Step) -> bool {
        match step {
            Step::Mend => self.mend.is_some(),
            Step::Relief => self.relief.is_some(),
            Step::Combine => self.combine.is_some(),
            Step::Label => self.label.is_some(),
            Step::Cut => self.cut.is_some(),
            Step::Split => self.split.is_some(),
            Step::Arrange => self.arrange,
            _ => false,
        }
    }
}

impl Shaped {
    /// Starts `step` on `ids`: the tools that take several models at once are started on all,
    /// the rest on the first, the others following as each lands.
    pub(super) fn start(
        step: Step,
        ids: &[ObjectId],
        shaping: &Shaping,
        (doc, tools, machine): (&mut Doc, &mut Tools, &mut Machine),
    ) -> Result<Self> {
        let before = doc.scene.objects().iter().map(|object| object.id).collect();
        let mut shaped = Self {
            step,
            shaping: shaping.clone(),
            left: ids.iter().copied().collect(),
            mending: None,
            before,
        };
        match step {
            Step::Relief | Step::Combine => {
                doc.scene.select_many(ids);
                shaped.left.clear();
                if step == Step::Relief {
                    start_relief(shaping, doc, tools, machine)?;
                } else {
                    tools.boolean.op = shaping.combine.unwrap_or_default();
                    tools.boolean.start(&doc.scene)?;
                }
            }
            Step::Arrange => {
                shaped.left.clear();
                let gap_mm = tools.array.gap_mm;
                machine.status = crate::arrange::arrange_plate(&mut doc.scene, &doc.plate, gap_mm);
            }
            _ => shaped.next(&mut doc.scene, tools)?,
        }
        Ok(shaped)
    }

    /// Whether the step has ended: its tool idle and no model left to start it on.
    pub(super) fn poll(&mut self, doc: &mut Doc, tools: &mut Tools) -> Result<bool> {
        if self.poll_mending(&mut doc.scene)? == Some(false) {
            return Ok(false);
        }
        let busy = match self.step {
            Step::Relief => tools.relief.is_busy(),
            Step::Combine => tools.boolean.is_busy(),
            Step::Label => tools.text.is_busy(),
            Step::Cut | Step::Split => tools.cut.is_busy(),
            _ => false,
        };
        if busy || self.mending.is_some() {
            return Ok(false);
        }
        if self.left.is_empty() {
            return Ok(true);
        }
        self.next(&mut doc.scene, tools)?;
        Ok(false)
    }

    /// The models of the run once the step has landed: those still standing and the pieces it
    /// put on the plate.
    pub(super) fn models_after(&self, ids: &[ObjectId], scene: &Scene) -> Vec<ObjectId> {
        let made = scene
            .objects()
            .iter()
            .map(|object| object.id)
            .filter(|id| !self.before.contains(id));
        let kept = ids.iter().copied().filter(|id| scene.get(*id).is_some());
        let mut models: Vec<ObjectId> = kept.chain(made).collect();
        models.dedup();
        models
    }

    /// Starts the step's tool on the next model left.
    fn next(&mut self, scene: &mut Scene, tools: &mut Tools) -> Result<()> {
        let Some(id) = self.left.pop_front() else {
            return Ok(());
        };
        let shaping = self.shaping.clone();
        match self.step {
            Step::Mend => {
                if let Some(mend) = &shaping.mend {
                    self.start_mending(id, mend, scene);
                }
            }
            Step::Label => {
                if let Some(label) = &shaping.label {
                    start_label(id, label, scene, tools)?;
                }
            }
            Step::Cut => {
                if let Some(cut) = shaping.cut {
                    start_cut(id, cut, scene, tools)?;
                }
            }
            Step::Split => {
                if let Some(split) = shaping.split {
                    start_split(id, split, scene, tools)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Queues the edits `mend` asks of model `id` in the command line's order — turns and
    /// bridges, then the drops, which renumber — and starts the first.
    fn start_mending(&mut self, id: ObjectId, mend: &Mend, scene: &Scene) {
        let Some(object) = scene.get(id) else {
            return;
        };
        let mut edits = VecDeque::new();
        if !mend.flip_faces.is_empty() {
            edits.push_back(Edit::Flip(region(&mend.flip_faces)));
        }
        edits.extend(mend.bridges.iter().map(|(a, b)| Edit::Bridge(*a, *b)));
        if !mend.drop_faces.is_empty() || !mend.drop_shells.is_empty() {
            let shells = mend
                .drop_shells
                .iter()
                .filter_map(|n| n.checked_sub(1))
                .collect();
            edits.push_back(Edit::Drop(region(&mend.drop_faces), shells));
        }
        if let Some(first) = edits.pop_front() {
            let job = EditJob::spawn(Arc::clone(&object.mesh), object.name.clone(), first);
            self.mending = Some((job, Taken::of(object), edits));
        }
    }

    /// Lands an edit that ended and starts the next; `None` with no edit going, else whether
    /// the edits are all done.
    fn poll_mending(&mut self, scene: &mut Scene) -> Result<Option<bool>> {
        let Some((job, taken, edits)) = self.mending.as_mut() else {
            return Ok(None);
        };
        let Some(outcome) = job.poll() else {
            return Ok(Some(false));
        };
        let object = scene
            .get_mut(taken.id)
            .filter(|object| taken.is_model(object));
        match (outcome, object) {
            (EditOutcome::Failed(message), _) => return Err(anyhow!(message)),
            (EditOutcome::Done(shaped, _), Some(object)) => object.reshape(*shaped),
            (EditOutcome::Done(..), None) => {
                return Err(anyhow!("the model changed under its edit"));
            }
        }
        let next = edits.pop_front();
        let Some(next) = next else {
            self.mending = None;
            return Ok(Some(true));
        };
        let Some(object) = scene.get(taken.id) else {
            self.mending = None;
            return Ok(Some(true));
        };
        *job = EditJob::spawn(Arc::clone(&object.mesh), object.name.clone(), next);
        *taken = Taken::of(object);
        Ok(Some(false))
    }
}

fn region(faces: &[u32]) -> Region {
    faces.iter().map(|face| *face as usize).collect()
}

/// The Relief tool set as `shaping` asks, its picture read where one is named, started on
/// the picked models.
fn start_relief(
    shaping: &Shaping,
    doc: &Doc,
    tools: &mut Tools,
    machine: &mut Machine,
) -> Result<()> {
    let Some(ask) = &shaping.relief else {
        return Ok(());
    };
    let relief = &mut tools.relief;
    relief.state.amplitude_mm = ask.depth_mm;
    relief.state.invert = ask.invert;
    relief.state.symmetric = ask.symmetric;
    relief.state.smoothing_px = ask.smooth_px.unwrap_or(0.0);
    relief.state.bottom_deg = ask.hold_bottom_deg.unwrap_or(0.0);
    relief.state.top_deg = ask.hold_top_deg.unwrap_or(0.0);
    relief.picture = None;
    if let Some(image) = &ask.image {
        crate::relief::load_picture(relief, &mut machine.status, &Handed::Path(image.clone()));
        if relief.picture.is_none() {
            return Err(anyhow!(
                "{} is not a picture the window reads",
                image.display()
            ));
        }
    }
    if let Some(tile_mm) = ask.tile_mm {
        relief.tiling.tile_mm = tile_mm;
    }
    if let Some(angle) = ask.tile_angle_deg {
        relief.tiling.angle_deg = angle;
    }
    relief.start(&doc.scene)
}

/// The Text tool set as `label` asks and stamped on model `id` at the surface nearest its point.
fn start_label(id: ObjectId, label: &LabelAsk, scene: &Scene, tools: &mut Tools) -> Result<()> {
    let object = scene.get(id).ok_or_else(|| anyhow!("the model is gone"))?;
    let hit = nearest_hit(object, label.at_mm)
        .ok_or_else(|| anyhow!("no surface near the label's point"))?;
    let state = &mut tools.text.state;
    state.text.clone_from(&label.text);
    state.height_mm = label.height_mm.unwrap_or(LETTER_HEIGHT_MM);
    state.depth_mm = label.depth_mm.unwrap_or(LABEL_DEPTH_MM);
    state.raised = !label.sunk;
    state.follow_surface = label.follow;
    state.angle_deg = label.angle_deg.unwrap_or(0.0);
    tools.text.stamp(scene, id, &hit)
}

/// The point of `object`'s surface nearest `at`, and the way it faces, as a click there finds.
pub(super) fn nearest_hit(object: &crate::scene::SceneObject, at: Vec3) -> Option<PlacedHit> {
    let inverse = object.transform.inverse_matrix()?;
    let found = object.bvh.closest(inverse.transform_point3(at))?;
    let world = object.transform.to_matrix();
    let normal = object.mesh.triangle(found.face)?.normal_unnormalized();
    Some(PlacedHit {
        point: world.transform_point3(found.point),
        normal: world
            .transform_vector3(normal)
            .try_normalize()
            .unwrap_or(Vec3::Z),
        distance: 0.0,
        face: found.face,
    })
}

/// The Cut tool set as `cut` asks and started on model `id`. Across X or Y the tool measures
/// from the model's centre of mass, the command line from the plate's corner.
fn start_cut(id: ObjectId, cut: CutAsk, scene: &Scene, tools: &mut Tools) -> Result<()> {
    let object = scene.get(id).ok_or_else(|| anyhow!("the model is gone"))?;
    let pivot = object.pivot().translation;
    let tool = &mut tools.cut;
    tool.state.axis = cut.axis;
    tool.state.height_mm = match cut.axis {
        Axis::X => cut.at_mm - pivot.x,
        Axis::Y => cut.at_mm - pivot.y,
        Axis::Z => cut.at_mm,
    };
    tool.centred_on = Some(id);
    tool.state.keep = cut.keep;
    tool.state.joints.kind = cut.joint;
    tool.lay_flat = false;
    tool.start(scene, id)
}

/// The split of model `id` into its parts, or of the part a point is on.
fn start_split(id: ObjectId, split: SplitAsk, scene: &Scene, tools: &mut Tools) -> Result<()> {
    let object = scene.get(id).ok_or_else(|| anyhow!("the model is gone"))?;
    match split {
        SplitAsk::Parts { smallest_mm3 } => {
            tools.cut.unmark_part();
            tools.cut.state.parts.smallest_mm3 = smallest_mm3;
        }
        SplitAsk::PartOff { at_mm } => tools.cut.mark_part(object, at_mm),
    }
    tools.cut.start_split(scene, id)
}
