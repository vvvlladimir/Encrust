//! The job a step of Prepare has going, which the run owns: no tool's own Cancel or start
//! reaches it, and its outcome comes back to the run rather than through the status line.

use anyhow::Result;

use super::plan::{Drains, Orient};
use super::{Chosen, Plan, Step};
use crate::i18n::tr;
use crate::job::{
    DrainsJob, DrainsOutcome, HollowJob, HollowOutcome, OrientJob, OrientOutcome, RepairJob,
    SupportJob, SupportOutcome,
};
use crate::orient::stand_turned;
use crate::plate::BuildPlate;
use crate::repair::RepairState;
use crate::scene::{ObjectId, Scene, SceneObject, Taken};
use crate::state::{Doc, Machine, Tools};
use crate::status::Status;
use crate::supports::{Side, Sizing, SupportTool};
use core_geometry::{Vec2, Vec3};
use core_volume::DEFAULT_BLOCKER_MM;
use core_volume::{HollowSettings, hole_at};

/// A step's job, running.
pub(super) enum Going {
    Orient(OrientJob),
    /// One repair a model, each with the model it took, and how many there were.
    Repair(Vec<(RepairJob, Taken)>, usize),
    Hollow(HollowJob),
    Drain(DrainsJob),
    /// Sized by the automatic profile where it carries one, built to a named profile where not,
    /// and the supports placed by hand that land with it.
    Supports(SupportJob, Side, Option<Box<Sizing>>, Vec<Vec3>),
    /// A step done as it started, with what it said: a turn given outright, holes drilled.
    Done(Status),
}

/// How a step's job ended: landed, with what it said, or stopped short with why.
pub(super) enum Ended {
    Landed(Status),
    Stopped(Status),
}

impl std::fmt::Debug for Going {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Going")
    }
}

impl Going {
    /// Starts `step` of `plan` on the models `chosen` names.
    pub(super) fn start(
        (step, plan): (Step, &Plan),
        chosen: &[Chosen],
        doc: &mut Doc,
        tools: &mut Tools,
        machine: &Machine,
    ) -> Result<Self> {
        let ids: Vec<ObjectId> = chosen.iter().map(|(id, _)| *id).collect();
        let layer_mm = machine.slicing.layer_height_mm();
        Ok(match step {
            Step::Orient => match plan.orient() {
                Orient::Turn(rotation) => {
                    let turned =
                        crate::orient::stand_rotated(&mut doc.scene, &doc.plate, &ids, rotation);
                    Self::Done(turned)
                }
                Orient::Search {
                    preset,
                    max_tilt_deg,
                } => Self::orient(&ids, (preset, max_tilt_deg), doc, tools, machine)?,
            },
            Step::Repair => Self::repair(&ids, plan, doc),
            Step::Hollow => {
                keep_solid(&ids, plan, &mut doc.scene);
                Self::hollow(chosen, plan, doc, tools)?
            }
            Step::Drill => match &plan.drains {
                Some(drains) => Self::Done(drill(&ids, drains, &mut doc.scene, tools)),
                None => Self::Done(Status::default()),
            },
            Step::Drain => {
                let vents = plan.drains.as_ref().is_none_or(|drains| drains.vent);
                Self::Drain(tools.drain.search_on(&doc.scene, &ids, layer_mm, vents)?)
            }
            Step::Supports | Step::Cavity => {
                let side = if step == Step::Cavity {
                    Side::Inside
                } else {
                    Side::Outside
                };
                Self::supports(&ids, (side, plan), doc, tools, machine)?
            }
        })
    }

    fn orient(
        ids: &[ObjectId],
        (preset, max_tilt_deg): (core_plate::OrientPreset, Option<f32>),
        doc: &Doc,
        tools: &mut Tools,
        machine: &Machine,
    ) -> Result<Self> {
        let slicing = &machine.slicing;
        let plans = tools.supports.automatic_plans(&slicing.catalogue)?;
        let proof = slicing
            .printer
            .as_ref()
            .map(|printer| (printer, &slicing.material));
        let asked = (preset, max_tilt_deg, plans);
        Ok(Self::Orient(
            tools.orient.start_on(&doc.scene, ids, asked, proof)?,
        ))
    }

    /// Each model at the settings given, else at Hollow's defaults round the wall decided.
    fn hollow(chosen: &[Chosen], plan: &Plan, doc: &Doc, tools: &Tools) -> Result<Self> {
        let given = plan.hollow_given();
        let models: Vec<(ObjectId, HollowSettings)> = chosen
            .iter()
            .filter_map(|(id, wall)| {
                let object = doc.scene.get(*id)?;
                let settings = match given {
                    Some(state) => crate::hollow::settings_of(state, object.transform),
                    None => HollowSettings::for_wall(wall.as_ref()?.wall_mm),
                };
                Some((*id, settings))
            })
            .collect();
        Ok(Self::Hollow(
            tools.hollow.start_settings(&doc.scene, &models)?,
        ))
    }

    /// Supports sized by the automatic profile, or built to the one the plan names: each
    /// model keeps that profile as its own for the group in hand, so they are drawn to it.
    fn supports(
        ids: &[ObjectId],
        (side, plan): (Side, &Plan),
        doc: &mut Doc,
        tools: &Tools,
        machine: &Machine,
    ) -> Result<Self> {
        let slicing = &machine.slicing;
        let layer_mm = slicing.layer_height_mm();
        let stand = plan.supports.clone().unwrap_or_default();
        let by_hand = if side == Side::Outside {
            stand.at_mm.clone()
        } else {
            Vec::new()
        };
        if stand.only {
            let group = tools.supports.active;
            return Ok(Self::Done(by_hand_on(ids, &by_hand, group, &mut doc.scene)));
        }
        let Some(named) = stand.profile.as_ref() else {
            let asked = (layer_mm, &slicing.catalogue);
            let (job, sizing) = SupportTool::start_automatic_on(&doc.scene, ids, asked, side)?;
            return Ok(Self::Supports(job, side, Some(Box::new(sizing)), by_hand));
        };
        let profile = slicing.catalogue.support(named)?.profile.clone();
        for id in ids {
            if let Some(object) = doc.scene.get_mut(*id) {
                object
                    .own
                    .supports
                    .insert(tools.supports.active, profile.clone());
            }
        }
        let job = SupportTool::start_profile_on(&doc.scene, ids, (layer_mm, &profile), side)?;
        Ok(Self::Supports(job, side, None, by_hand))
    }

    fn repair(ids: &[ObjectId], plan: &Plan, doc: &Doc) -> Self {
        let state = RepairState {
            method: plan.method(),
            ..RepairState::default()
        };
        let jobs: Vec<(RepairJob, Taken)> = ids
            .iter()
            .filter_map(|id| crate::repair::spawn(&doc.scene, *id, state))
            .collect();
        let count = jobs.len();
        Self::Repair(jobs, count)
    }

    /// The share of the step's job done, where it counts it.
    pub(super) fn fraction(&self) -> Option<f32> {
        match self {
            Self::Orient(job) => job.counted().map(|(_, share)| share),
            Self::Repair(jobs, count) => {
                let going: f32 = jobs
                    .iter()
                    .filter_map(|(job, _)| job.counted())
                    .map(|counted| counted.share)
                    .sum();
                let done = count.saturating_sub(jobs.len()) as f32;
                Some((done + going) / (*count).max(1) as f32)
            }
            Self::Hollow(job) => job.fraction(),
            Self::Drain(job) => Some(job.fraction()),
            Self::Supports(job, ..) => job.fraction(),
            Self::Done(_) => Some(1.0),
        }
    }

    /// Lands the job into the plate once it has ended, and `None` while it runs.
    pub(super) fn poll(&mut self, doc: &mut Doc, tools: &mut Tools) -> Option<Ended> {
        let scene = &mut doc.scene;
        match self {
            Self::Orient(job) => Some(match job.poll()? {
                OrientOutcome::Turned(turns) => {
                    Ended::Landed(stand_turned(scene, &doc.plate, &turns))
                }
                OrientOutcome::Failed(message) => Ended::Stopped(Status::Error(message)),
            }),
            Self::Repair(jobs, _) => Self::poll_repairs(jobs, scene),
            Self::Done(said) => Some(Ended::Landed(std::mem::take(said))),
            Self::Hollow(job) => Some(match job.poll()? {
                outcome @ HollowOutcome::Hollowed(_) => {
                    Ended::Landed(tools.hollow.land(scene, outcome))
                }
                outcome => Ended::Stopped(tools.hollow.land(scene, outcome)),
            }),
            Self::Drain(job) => Some(match job.poll()? {
                DrainsOutcome::Found(found) => Ended::Landed(tools.drain.land_found(scene, found)),
                DrainsOutcome::Cancelled => {
                    Ended::Stopped(Status::Info(tr!("drain-auto-cancelled")))
                }
                DrainsOutcome::Failed(message) => Ended::Stopped(Status::Error(message)),
            }),
            Self::Supports(job, side, sizing, by_hand) => Some(match job.poll()? {
                SupportOutcome::Placed(placements) => {
                    let sizing = sizing.as_deref().cloned();
                    let ids: Vec<ObjectId> = placements.iter().map(|placed| placed.id).collect();
                    let said = tools.supports.land_placed(scene, placements, *side, sizing);
                    by_hand_on(&ids, by_hand, tools.supports.active, scene);
                    Ended::Landed(said)
                }
                SupportOutcome::Cancelled => {
                    Ended::Stopped(Status::Info(tr!("supports-cancelled")))
                }
                SupportOutcome::Failed(message) => Ended::Stopped(Status::Error(message)),
            }),
        }
    }

    /// Lands each repair that ended; a repair that could not mend is said and the run goes
    /// on, as the command line's does.
    fn poll_repairs(
        jobs: &mut Vec<(RepairJob, Taken)>,
        scene: &mut crate::scene::Scene,
    ) -> Option<Ended> {
        let mut said = None;
        jobs.retain_mut(|(job, taken)| match job.poll() {
            None => true,
            Some(outcome) => {
                said = Some(crate::repair::land(scene, taken.clone(), outcome));
                false
            }
        });
        jobs.is_empty()
            .then(|| Ended::Landed(said.unwrap_or_default()))
    }
}

/// Drills a hole into the surface nearest each of `drains`' points on each model, at its size
/// or the Drain tool's, as `--drain-at` drills: kept as the model's own size, so its plugs and
/// any search after follow it. Says how many were drilled.
fn drill(ids: &[ObjectId], drains: &Drains, scene: &mut Scene, tools: &Tools) -> Status {
    let mut drilled = 0;
    for id in ids {
        let Some(object) = scene.get_mut(*id) else {
            continue;
        };
        if let Some(size) = &drains.size {
            object.own.drain = Some(size.clone());
        }
        let Some(inverse) = object.transform.inverse_matrix() else {
            continue;
        };
        let least = object.transform.least_scale();
        let mut size = tools.drain.size_for(object);
        size.diameter_mm /= least;
        size.depth_mm /= least;
        for at in &drains.at_mm {
            let near = inverse.transform_point3(*at);
            if let Some(hole) = hole_at(&object.mesh, &object.bvh, near, size) {
                object.hollow.add_hole(hole);
                drilled += 1;
            }
        }
    }
    Status::Info(tr!("prepare-drilled", count = drilled))
}

/// Scales `object` by `scale` from the way it came in, as `--scale` does before anything else,
/// and stands it back on the middle of the plate.
pub(super) fn scale(object: &mut SceneObject, scale: Vec3, plate: &BuildPlate) {
    object.transform.scale = scale;
    object.slide_onto_plate(Vec2::new(plate.x_mm, plate.y_mm));
    object.stand_on_plate();
}

/// Keeps the wall solid round each of the plan's points on the models it hollows, at the
/// blocker radius of the hollow given or Hollow's default.
fn keep_solid(ids: &[ObjectId], plan: &Plan, scene: &mut Scene) {
    let radius_mm = plan
        .hollow_given()
        .map_or(DEFAULT_BLOCKER_MM, |state| state.blocker_mm);
    for id in ids {
        if let Some(object) = scene.get_mut(*id) {
            let transform = object.transform;
            for at in &plan.keep_solid {
                object.hollow.add_blocker(*at, radius_mm, transform);
            }
        }
    }
}

/// Stands a support in `group` under the surface of each model nearest each of `at_mm`, as a
/// click in Supports' Edit mode does. Says how many.
fn by_hand_on(ids: &[ObjectId], at_mm: &[Vec3], group: u16, scene: &mut Scene) -> Status {
    let mut placed = 0;
    for id in ids {
        let Some(object) = scene.get_mut(*id) else {
            continue;
        };
        let transform = object.transform;
        let (Some(inverse), world) = (transform.inverse_matrix(), transform.to_matrix()) else {
            continue;
        };
        for at in at_mm {
            if let Some(found) = object.bvh.closest(inverse.transform_point3(*at)) {
                object
                    .supports
                    .add(world.transform_point3(found.point), transform, group);
                placed += 1;
            }
        }
    }
    Status::Info(tr!("supports-placed", count = placed))
}
