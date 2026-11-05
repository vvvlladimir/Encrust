//! A run moving through its plan: each step decided per model on the plate as it stands,
//! its models batched by what they run, each batch's job started and its landing noted.

use std::collections::VecDeque;

use anyhow::Result;
use core_engine::Hollowing;
use core_recipe::{Decision, Metric, OnFail, Plan, StepKind, decide};

use super::step::Going;
use super::work::{self, Batch, NotYet, Settings, Work};
use super::{IDLE_POLLS, Prepared, Run, Step};
use crate::i18n::tr;
use crate::job::Highest;
use crate::plate::BuildPlate;
use crate::scene::{ObjectId, Scene};
use crate::state::{Doc, Machine, Tools};
use crate::status::Status;
use crate::supports::{BuiltTo, Side};

/// Where a run stands once it has moved on.
pub(super) enum Next {
    /// A job is running, or the plate is being cut for the next step.
    Working,
    Done,
    /// A gate stopped it, or it reached a step it cannot run: the plate goes back.
    Stopped(Status),
}

/// What a step decided for the run's models.
enum Decided {
    Batches(Vec<Batch>),
    Stop(Status),
    /// Wait for Continue, saying this.
    Pause(String),
}

impl Run {
    pub(super) fn new(plan: Plan, ids: Vec<ObjectId>) -> Self {
        Self {
            prepared: ids.iter().map(|id| Prepared::new(*id)).collect(),
            ids,
            paused: None,
            plan,
            next: 0,
            index: 0,
            queue: VecDeque::new(),
            going: None,
            step: Step::Orient,
            measured: Vec::new(),
            supported: Vec::new(),
            built: Vec::new(),
            unplugged: Vec::new(),
            idle: 0,
            done: 0,
            steps: 0,
            shown: Highest::default(),
        }
    }

    /// Whether the step in hand moves on when its job fails rather than ending the run.
    pub(super) fn skips(&self) -> bool {
        let step = self.plan.recipe().steps.get(self.index);
        step.is_some_and(|step| step.on_fail == OnFail::Skip)
    }

    /// Starts the next batch that has anything to do, deciding steps as they are reached.
    pub(super) fn advance(
        &mut self,
        doc: &mut Doc,
        tools: &mut Tools,
        machine: &mut Machine,
    ) -> Result<Next> {
        loop {
            let Some(batch) = self.queue.pop_front() else {
                if let Some(next) = self.refill(doc)? {
                    return Ok(next);
                }
                continue;
            };
            if let Some(next) = self.in_frame(&batch, doc, machine) {
                return Ok(next);
            }
            match Going::start(&batch, doc, tools, machine) {
                Ok(Some(going)) => {
                    self.begin(batch, going, doc);
                    return Ok(Next::Working);
                }
                Ok(None) => self.batch_ended(),
                Err(error) if self.skips() => {
                    machine.status = Status::failed(&error);
                    self.queue.clear();
                    self.batch_ended();
                }
                Err(error) => return Err(error),
            }
        }
    }

    /// Queues the batches of the next plan step, or says where the run stands instead: at
    /// its end, stopped by a gate, or waiting at a pause.
    fn refill(&mut self, doc: &Doc) -> Result<Option<Next>> {
        if self.next >= self.plan.recipe().steps.len() {
            return Ok(Some(Next::Done));
        }
        Ok(match self.decide_next(&doc.scene, &doc.plate)? {
            Decided::Batches(batches) => {
                self.queue = batches.into();
                None
            }
            Decided::Stop(why) => Some(Next::Stopped(why)),
            Decided::Pause(message) => {
                self.step = Step::Pause;
                self.paused = Some(message);
                Some(Next::Working)
            }
        })
    }

    /// Does what `batch` asks that takes no job, in this frame, answering where that leaves
    /// the run; `None` where its job is still to start.
    fn in_frame(&mut self, batch: &Batch, doc: &mut Doc, machine: &mut Machine) -> Option<Next> {
        match &batch.work {
            Work::DrainPoints { .. } => {
                self.drill_points(&mut doc.scene, batch);
                self.batch_ended();
                Some(Next::Working)
            }
            Work::Arrange { spacing_mm } => {
                let said = crate::arrange::arrange_plate(&mut doc.scene, &doc.plate, *spacing_mm);
                let stopped = self.ended_by(said, &mut machine.status);
                Some(stopped.map_or(Next::Working, Next::Stopped))
            }
            Work::Supports {
                side: Side::Outside,
                ..
            } => {
                self.take_off_earlier(&mut doc.scene, &batch.ids);
                None
            }
            Work::Scan => {
                self.note_solid(&doc.scene, &batch.ids);
                None
            }
            _ => None,
        }
    }

    /// Ends a step done in the frame it was reached, as `said` tells: on to the next, or
    /// the run's end when it failed and does not skip.
    fn ended_by(&mut self, said: Status, status: &mut Status) -> Option<Status> {
        if said.is_error() && !self.skips() {
            return Some(said);
        }
        if said.is_error() {
            self.queue.clear();
        }
        *status = said;
        self.batch_ended();
        None
    }

    /// No pocket to trap resin in a model with no cavity: `trapped` is nought.
    fn note_solid(&mut self, scene: &Scene, ids: &[ObjectId]) {
        let solid: Vec<ObjectId> = scene
            .named(ids)
            .filter(|object| !object.hollow.is_hollow())
            .map(|object| object.id)
            .collect();
        for id in solid {
            self.note(id, Metric::Trapped, 0.0);
        }
    }

    fn begin(&mut self, batch: Batch, going: Going, doc: &Doc) {
        self.step = batch.work.step();
        self.mark(&batch, &doc.scene);
        self.going = Some((batch, going));
        self.recount(&doc.scene, &doc.plate);
    }

    /// Decides the next plan step for each model of the run.
    fn decide_next(&mut self, scene: &Scene, plate: &BuildPlate) -> Result<Decided> {
        let index = self.next;
        self.index = index;
        self.next += 1;
        let kind = self.plan.recipe().steps[index].kind;
        let mut runs: Vec<(ObjectId, Settings)> = Vec::new();
        for object in scene.named(&self.ids) {
            match decide(&self.plan, index, &self.metrics_of(object, plate))? {
                Decision::Run { settings, .. } => runs.push((object.id, settings)),
                Decision::Stop { message, .. } if message.is_empty() => {
                    return Ok(Decided::Stop(Status::Error(tr!("prepare-gate-stopped"))));
                }
                Decision::Stop { message, .. } => return Ok(Decided::Stop(Status::Error(message))),
                Decision::Pause { message } if message.is_empty() => {
                    return Ok(Decided::Pause(tr!("prepare-paused")));
                }
                Decision::Pause { message } => return Ok(Decided::Pause(message)),
                Decision::Skip { .. } | Decision::Disabled => {}
            }
        }
        match batches(kind, runs) {
            Ok(batches) => Ok(Decided::Batches(batches)),
            Err(not) => Ok(Decided::Stop(not_yet(not))),
        }
    }

    /// Ends the batch in hand, and the step with it once it has no batch left.
    pub(super) fn batch_ended(&mut self) {
        if self.queue.is_empty() && self.going.is_none() {
            self.done += 1;
        }
    }

    /// Whether the holes a step drilled are still being cut, or the surface supports stand
    /// on measured, which the next step works on. Only a settled frame counts as idle: the
    /// window starts no cutting while a button is down.
    pub(super) fn settling(&mut self, doc: &Doc, settled: bool) -> bool {
        let unsettled = doc.scene.named(&self.ids).any(|object| {
            object.hollow.wants_drilling(object.uncut())
                || object.unbuilt_support_surface().is_some()
        });
        if !unsettled || doc.drilling.is_running() || doc.surfaces.is_running() {
            self.idle = 0;
            return unsettled;
        }
        if settled {
            self.idle += 1;
        }
        self.idle < IDLE_POLLS
    }

    /// Notes the models `batch` names as its step found them.
    fn mark(&mut self, batch: &Batch, scene: &Scene) {
        let wall = match &batch.work {
            Work::Hollow(hollowed) => Some(hollowed.wall_mm),
            _ => None,
        };
        for id in &batch.ids {
            let prepared = self.prepared.iter_mut().find(|prepared| prepared.id == *id);
            if let Some((prepared, object)) = prepared.zip(scene.get(*id)) {
                let linear = object.transform.linear();
                let wall = wall.map(|wall_mm| Hollowing::reckoned(&object.mesh, linear, wall_mm));
                prepared.started(batch.work.step(), object, wall);
            }
        }
    }

    /// Notes what the batch just landed changed on the plate, what a drain left trapped, and
    /// the supports to raise inside the cavities after those outside.
    pub(super) fn landed(&mut self, scene: &Scene, batch: Batch, built: Option<&BuiltTo>) {
        let step = batch.work.step();
        let profile = built.map(BuiltTo::name);
        if let Some(built) = built {
            for id in &batch.ids {
                self.built.retain(|(owner, _)| owner != id);
                self.built.push((*id, built.clone()));
            }
        }
        for prepared in &mut self.prepared {
            if let Some(object) = scene.get(prepared.id) {
                prepared.landed(step, object, profile.unwrap_or_default());
            }
        }
        if let Work::DrainAuto { .. } | Work::Scan = batch.work {
            let found: Vec<(ObjectId, usize)> = scene
                .named(&batch.ids)
                .filter(|object| object.hollow.is_hollow())
                .map(|object| (object.id, object.traps.found().len()))
                .collect();
            for (id, trapped) in found {
                self.note(id, Metric::Trapped, trapped as f32);
            }
        }
        if matches!(batch.work.step(), Step::Drain) {
            self.unplugged.retain(|id| !batch.ids.contains(id));
            if batch.work.unplugged() {
                self.unplugged.extend(&batch.ids);
            }
        }
        if let Work::DrainPoints { .. } = batch.work {
            self.queue
                .push_front(Batch::of(Work::Scan, batch.ids.clone()));
        }
        if let Work::Supports {
            profile,
            side: Side::Outside,
        } = batch.work
        {
            self.supported.extend(&batch.ids);
            if !batch.inside.is_empty() {
                let inside = Work::Supports {
                    profile,
                    side: Side::Inside,
                };
                self.queue.push_front(Batch::of(inside, batch.inside));
            }
        }
    }

    /// Keeps the window's plugs off the holes of the models a drain step left unplugged, as
    /// they stand now: a later step may deepen them.
    pub(super) fn leave_unplugged(&self, doc: &mut Doc, tools: &Tools, machine: &Machine) {
        let drift_mm = machine.slicing.material.compensation.fit_drift_mm();
        for id in &self.unplugged {
            tools.drain.leave_unplugged(&mut doc.scene, *id, drift_mm);
        }
    }

    /// Takes off the supports an earlier supports step of this run stood `ids` on.
    fn take_off_earlier(&self, scene: &mut Scene, ids: &[ObjectId]) {
        for id in ids.iter().filter(|id| self.supported.contains(id)) {
            if let Some(object) = scene.get_mut(*id) {
                object.supports.clear();
            }
        }
    }

    /// Drills a hole at each of the batch's points, as a click drills one.
    fn drill_points(&mut self, scene: &mut Scene, batch: &Batch) {
        let Work::DrainPoints { size, points, .. } = &batch.work else {
            return;
        };
        self.step = Step::Drain;
        self.mark(batch, scene);
        for id in &batch.ids {
            if let Some(object) = scene.get_mut(*id) {
                crate::drain::drill_at_fractions(object, points, *size);
            }
        }
        self.landed(scene, batch.clone(), None);
    }
}

/// The runs of step `kind`, gathered by the work each asks; a step on the whole plate is
/// one batch, as its first model asks it, and a check reads the drainage first.
fn batches(kind: StepKind, runs: Vec<(ObjectId, Settings)>) -> Result<Vec<Batch>, NotYet> {
    let mut batches = gathered(kind, runs)?;
    if batches
        .first()
        .is_some_and(|first| first.work.whole_plate())
    {
        let ids: Vec<ObjectId> = batches.iter().flat_map(|batch| batch.ids.clone()).collect();
        batches.truncate(1);
        batches[0].ids.clone_from(&ids);
        if kind == StepKind::Check {
            batches.insert(0, Batch::of(Work::Scan, ids));
        }
    }
    Ok(batches)
}

fn gathered(kind: StepKind, runs: Vec<(ObjectId, Settings)>) -> Result<Vec<Batch>, NotYet> {
    let mut batches: Vec<Batch> = Vec::new();
    for (id, settings) in runs {
        let work = Work::of(kind, &settings)?;
        let inside = work::cavity(&settings).then_some(id);
        match batches.iter_mut().find(|batch| batch.work == work) {
            Some(batch) => {
                batch.ids.push(id);
                batch.inside.extend(inside);
            }
            None => batches.push(Batch {
                work,
                ids: vec![id],
                inside: inside.into_iter().collect(),
            }),
        }
    }
    Ok(batches)
}

fn not_yet(NotYet(kind): NotYet) -> Status {
    Status::Error(tr!("prepare-step-not-yet", step = kind.name()))
}
