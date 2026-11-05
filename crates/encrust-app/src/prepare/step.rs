//! The job a step of Prepare has going, which the run owns: no tool's own Cancel or start
//! reaches it, and its outcome comes back to the run rather than through the status line.

use anyhow::Result;

use super::check::Checking;
use super::save::Saving;
use super::work::{Batch, Work};
use crate::drain::DrainTool;
use crate::i18n::tr;
use crate::job::{
    DrainsJob, DrainsOutcome, HollowJob, HollowOutcome, OrientJob, OrientOutcome, RepairJob,
    SupportJob, SupportOutcome, TrapJob, TrapOutcome,
};
use crate::orient::stand_turned;
use crate::repair::RepairState;
use crate::scene::{ObjectId, Taken};
use crate::state::{Doc, Machine, Tools};
use crate::status::Status;
use crate::supports::{BuiltTo, Side, SupportTool};

/// A step's job, running.
pub(super) enum Going {
    Orient(OrientJob),
    /// One repair a model, each with the model it took, and how many there were.
    Repair(Vec<(RepairJob, Taken)>, usize),
    Hollow(HollowJob),
    Drain(DrainsJob),
    Supports(SupportJob, Side, Box<BuiltTo>),
    Scan(TrapJob),
    Check(Checking),
    Save(Saving),
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
    /// Starts `batch`'s job, or `None` where none of its models has anything for it to do:
    /// no cavity to drain or to hold up.
    pub(super) fn start(
        batch: &Batch,
        doc: &Doc,
        tools: &mut Tools,
        machine: &Machine,
    ) -> Result<Option<Self>> {
        let (scene, slicing) = (&doc.scene, &machine.slicing);
        let layer_mm = slicing.layer_height_mm();
        let ids = &batch.ids;
        let hollow: Vec<ObjectId> = scene
            .named(ids)
            .filter(|object| object.hollow.is_hollow())
            .map(|object| object.id)
            .collect();
        Ok(Some(match &batch.work {
            Work::Orient {
                preset,
                max_tilt_deg,
            } => {
                let plans = tools.supports.automatic_plans(&slicing.catalogue)?;
                let proof = slicing
                    .printer
                    .as_ref()
                    .map(|printer| (printer, &slicing.material));
                let asked = (*preset, *max_tilt_deg, plans);
                Self::Orient(tools.orient.start_on(scene, ids, asked, proof)?)
            }
            Work::Repair(method) => Self::repair(ids, doc, *method),
            Work::Hollow(hollowed) => {
                let asked: Vec<_> = scene
                    .named(ids)
                    .map(|object| (object.id, hollowed.settings_for(object.transform)))
                    .collect();
                Self::Hollow(tools.hollow.start_asked(scene, &asked)?)
            }
            Work::DrainAuto { .. } if hollow.is_empty() => {
                return Ok(None);
            }
            Work::DrainAuto { size, vent, .. } => {
                let asked = (*size, *vent);
                Self::Drain(DrainTool::search_on(scene, &hollow, layer_mm, asked)?)
            }
            // Drilled where it is decided, in one frame, by `drill_points`.
            Work::DrainPoints { .. } => return Ok(None),
            Work::Supports { profile, side } => {
                let held: Vec<ObjectId> = match side {
                    Side::Outside => ids.clone(),
                    Side::Inside => scene
                        .named(ids)
                        .filter(|object| object.supported_inside())
                        .map(|object| object.id)
                        .collect(),
                };
                if held.is_empty() {
                    return Ok(None);
                }
                let built = BuiltTo::of(profile, &slicing.catalogue)?;
                let job = SupportTool::start_built_on(scene, &held, (layer_mm, &built), *side)?;
                Self::Supports(job, *side, Box::new(built))
            }
            Work::Scan if hollow.is_empty() => return Ok(None),
            Work::Scan => Self::Scan(DrainTool::scan_on(scene, &hollow, layer_mm)?),
            Work::Check { .. } => Self::Check(Checking::default()),
            // Done where it is decided, in one frame, by `advance`.
            Work::Arrange { .. } => return Ok(None),
            Work::Save(saved) => Self::Save(Saving::new(saved, scene, machine)?),
        }))
    }

    fn repair(ids: &[ObjectId], doc: &Doc, method: core_volume::RepairMethod) -> Self {
        let state = RepairState {
            method,
            ..RepairState::default()
        };
        let jobs: Vec<(RepairJob, Taken)> = ids
            .iter()
            .filter_map(|id| crate::repair::spawn(&doc.scene, *id, state))
            .collect();
        let count = jobs.len();
        Self::Repair(jobs, count)
    }

    /// What a supports job builds to.
    pub(super) fn built(&self) -> Option<&BuiltTo> {
        match self {
            Self::Supports(_, _, built) => Some(built),
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
            Self::Scan(job) => job.fraction(),
            Self::Check(_) | Self::Save(_) => None,
        }
    }

    /// Lands the job into the plate once it has ended, and `None` while it runs.
    pub(super) fn poll(
        &mut self,
        doc: &mut Doc,
        tools: &mut Tools,
        machine: &mut Machine,
    ) -> Option<Ended> {
        match self {
            Self::Check(checking) => return checking.poll(doc, machine),
            Self::Save(saving) => return saving.poll(&doc.scene, machine),
            _ => {}
        }
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
            Self::Supports(job, side, built) => Some(match job.poll()? {
                SupportOutcome::Placed(placements) => {
                    let built = (**built).clone();
                    Ended::Landed(tools.supports.land_built(scene, placements, *side, built))
                }
                SupportOutcome::Cancelled => {
                    Ended::Stopped(Status::Info(tr!("supports-cancelled")))
                }
                SupportOutcome::Failed(message) => Ended::Stopped(Status::Error(message)),
            }),
            Self::Scan(job) => Some(match job.poll()? {
                TrapOutcome::Checked(models) => {
                    let found = crate::drain::land_checked(scene, models);
                    Ended::Landed(Status::Info(tr!("drain-found", count = found)))
                }
                TrapOutcome::Failed(message) => Ended::Stopped(Status::Error(message)),
            }),
            Self::Check(_) | Self::Save(_) => None,
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
