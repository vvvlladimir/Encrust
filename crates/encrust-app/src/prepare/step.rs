//! The job a step of Prepare has going, which the run owns: no tool's own Cancel or start
//! reaches it, and its outcome comes back to the run rather than through the status line.

use anyhow::Result;

use super::plan::{Drains, Orient, Wish};
use super::{Chosen, Plan, Step};
use crate::i18n::tr;
use crate::job::{
    DrainsJob, DrainsOutcome, HollowJob, HollowOutcome, OrientJob, OrientOutcome, RepairJob,
    SupportJob, SupportOutcome,
};
use crate::orient::stand_turned;
use crate::repair::RepairState;
use crate::scene::{ObjectId, Scene, Taken};
use crate::state::{Doc, Machine, Tools};
use crate::status::Status;
use crate::supports::{Side, Sizing, SupportTool};
use core_geometry::Vec3;
use core_volume::HollowSettings;

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
    /// A modelling step, done by the window's own tool, which says what it did.
    Shaped(Box<super::shape::Shaped>),
    /// Holes to drill into the supports of these models at these points, once built.
    SupportHoles(Vec<ObjectId>, Vec<Vec3>),
}

/// How a step's job ended: landed, with what it said, or stopped short with why.
pub(super) enum Ended {
    Landed(Status),
    /// Landed, the tool that did it having said what it did.
    Kept,
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
        machine: &mut Machine,
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
                    wish,
                } => Self::orient(&ids, (preset, max_tilt_deg, &wish), doc, tools, machine)?,
            },
            Step::Repair => Self::repair(&ids, plan, doc),
            Step::Hollow => {
                super::placed::keep_solid(&ids, plan, &mut doc.scene);
                if let Some(drains) = plan.drains.as_ref().filter(|drains| drains.walled) {
                    let points = Drains {
                        channels: Vec::new(),
                        ..drains.clone()
                    };
                    super::placed::drill(&ids, &points, &mut doc.scene, tools);
                }
                Self::hollow(chosen, plan, doc, tools)?
            }
            Step::Drill => match &plan.drains {
                Some(drains) if drains.walled => {
                    let channels = Drains {
                        at_mm: Vec::new(),
                        ..drains.clone()
                    };
                    Self::Done(super::placed::drill(&ids, &channels, &mut doc.scene, tools))
                }
                Some(drains) => {
                    Self::Done(super::placed::drill(&ids, drains, &mut doc.scene, tools))
                }
                None => Self::Done(Status::default()),
            },
            Step::SupportHoles => {
                let drains = plan.drains.as_ref();
                Self::SupportHoles(ids, drains.map_or_else(Vec::new, |d| d.support_at.clone()))
            }
            Step::Drain => {
                let vents = plan.drains.as_ref().is_none_or(|drains| drains.vent);
                Self::Drain(tools.drain.search_on(&doc.scene, &ids, layer_mm, vents)?)
            }
            Step::Mend
            | Step::Relief
            | Step::Combine
            | Step::Label
            | Step::Cut
            | Step::Split
            | Step::Arrange => {
                let shaping = &plan.shape;
                let asked = (&mut *doc, &mut *tools, machine);
                Self::Shaped(Box::new(super::shape::Shaped::start(
                    step, &ids, shaping, asked,
                )?))
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
        (preset, max_tilt_deg, wish): (core_plate::OrientPreset, Option<f32>, &Wish),
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
        Ok(Self::Orient(tools.orient.start_on(
            &doc.scene,
            ids,
            asked,
            (wish, proof),
        )?))
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
            return Ok(Self::Done(super::placed::by_hand_on(
                ids,
                &by_hand,
                group,
                &mut doc.scene,
            )));
        }
        let Some(named) = stand.profile.as_ref() else {
            let asked = (layer_mm, &slicing.catalogue);
            let (job, sizing) = SupportTool::start_automatic_on(&doc.scene, ids, asked, side)?;
            return Ok(Self::Supports(job, side, Some(Box::new(sizing)), by_hand));
        };
        let profile = if named.to_ascii_lowercase().ends_with(".toml") {
            printer_profiles::SupportProfile::load(std::path::Path::new(named))?
        } else {
            slicing.catalogue.support(named)?.profile.clone()
        };
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

    /// The run's models once this step has landed, where it is one that makes pieces in place
    /// of the models it took.
    pub(super) fn models_after(&self, ids: &[ObjectId], scene: &Scene) -> Option<Vec<ObjectId>> {
        match self {
            Self::Shaped(shaped) => Some(shaped.models_after(ids, scene)),
            _ => None,
        }
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
            Self::Shaped(_) | Self::SupportHoles(..) => None,
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
            Self::SupportHoles(ids, at) => super::placed::support_holes(ids, at, scene, tools),
            Self::Shaped(shaped) => match shaped.poll(doc, tools) {
                Ok(false) => None,
                Ok(true) => Some(Ended::Kept),
                Err(error) => Some(Ended::Stopped(Status::failed(&error))),
            },
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
                    super::placed::by_hand_on(&ids, by_hand, tools.supports.active, scene);
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
