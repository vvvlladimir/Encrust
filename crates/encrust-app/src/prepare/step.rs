//! The job a step of Prepare has going, which the run owns: no tool's own Cancel or start
//! reaches it, and its outcome comes back to the run rather than through the status line.

use anyhow::Result;

use super::{Chosen, Plan, Step};
use crate::i18n::tr;
use crate::job::{
    DrainsJob, DrainsOutcome, HollowJob, HollowOutcome, OrientJob, OrientOutcome, RepairJob,
    SupportJob, SupportOutcome,
};
use crate::orient::stand_turned;
use crate::repair::RepairState;
use crate::scene::{ObjectId, Taken};
use crate::state::{Doc, Machine, Tools};
use crate::status::Status;
use crate::supports::{Side, Sizing, SupportTool};

/// A step's job, running.
pub(super) enum Going {
    Orient(OrientJob),
    /// One repair a model, each with the model it took, and how many there were.
    Repair(Vec<(RepairJob, Taken)>, usize),
    Hollow(HollowJob),
    Drain(DrainsJob),
    Supports(SupportJob, Side, Box<Sizing>),
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
        (step, plan): (Step, Plan),
        chosen: &[Chosen],
        doc: &Doc,
        tools: &mut Tools,
        machine: &Machine,
    ) -> Result<Self> {
        let ids: Vec<ObjectId> = chosen.iter().map(|(id, _)| *id).collect();
        let (scene, slicing) = (&doc.scene, &machine.slicing);
        let layer_mm = slicing.layer_height_mm();
        Ok(match step {
            Step::Orient => Self::orient(&ids, plan, doc, tools, machine)?,
            Step::Repair => Self::repair(&ids, plan, doc),
            Step::Hollow => {
                let walls: Vec<(ObjectId, f32)> = chosen
                    .iter()
                    .filter_map(|(id, wall)| Some((*id, wall.as_ref()?.wall_mm)))
                    .collect();
                Self::Hollow(tools.hollow.start_walls(scene, &walls)?)
            }
            Step::Drain => Self::Drain(tools.drain.search_on(scene, &ids, layer_mm)?),
            Step::Supports | Step::Cavity => {
                let side = if step == Step::Cavity {
                    Side::Inside
                } else {
                    Side::Outside
                };
                let asked = (layer_mm, &slicing.catalogue);
                let (job, sizing) = SupportTool::start_automatic_on(scene, &ids, asked, side)?;
                Self::Supports(job, side, Box::new(sizing))
            }
        })
    }

    fn orient(
        ids: &[ObjectId],
        plan: Plan,
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
        let asked = (plan.preset(), plans);
        Ok(Self::Orient(
            tools.orient.start_on(&doc.scene, ids, asked, proof)?,
        ))
    }

    fn repair(ids: &[ObjectId], plan: Plan, doc: &Doc) -> Self {
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
            Self::Supports(job, side, sizing) => Some(match job.poll()? {
                SupportOutcome::Placed(placements) => {
                    let sizing = Some((**sizing).clone());
                    Ended::Landed(tools.supports.land_placed(scene, placements, *side, sizing))
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
