//! Prepare for print: the decisions `--prepare` makes, taken on the picked models by starting
//! each tool's own job in turn and waiting for it to land, all one undo (ADR 0399).

use anyhow::{Result, bail};
use core_engine::{Hollowing, hollowing, needs_repair};
use core_geometry::Quat;

use crate::i18n::tr;
use crate::job::Highest;
use crate::scene::{ObjectId, Scene, SceneObject};
use crate::state::{Doc, Machine, Tools};
use crate::status::Status;
use crate::tool_settings::ToolSettings;

mod decided;
mod step;

pub use decided::Prepared;
use step::{Ended, Going};

/// Settled polls the plate may wait on cuts or a support surface no job is building, which the
/// window starts a frame after a landing, before a step starts anyway: a failed one never restarts.
const IDLE_POLLS: u8 = 3;

/// A turn this small, radians, is a model nobody turned: the window's `--rotate` left out.
const UNTURNED_RAD: f32 = 1e-4;

/// The steps in the order they run, the command line's: turned before it is repaired.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Orient,
    Repair,
    Hollow,
    Drain,
    Supports,
    Cavity,
}

impl Step {
    const ALL: [Self; 6] = [
        Self::Orient,
        Self::Repair,
        Self::Hollow,
        Self::Drain,
        Self::Supports,
        Self::Cavity,
    ];

    fn index(self) -> usize {
        Self::ALL.iter().position(|step| *step == self).unwrap_or(0)
    }

    fn next(self) -> Option<Self> {
        Self::ALL.get(self.index() + 1).copied()
    }

    /// What the bar says while it runs.
    pub fn label(self) -> String {
        match self {
            Self::Orient => tr!("prepare-step-orient"),
            Self::Repair => tr!("prepare-step-repair"),
            Self::Hollow => tr!("prepare-step-hollow"),
            Self::Drain => tr!("prepare-step-drain"),
            Self::Supports => tr!("prepare-step-supports"),
            Self::Cavity => tr!("prepare-step-cavity"),
        }
    }
}

/// The run going, if any, and what the last one decided.
#[derive(Debug, Default)]
pub struct Prepare {
    run: Option<Run>,
    prepared: Vec<Prepared>,
}

/// One run: the models it took, the step in hand and what has been decided so far.
#[derive(Debug)]
struct Run {
    ids: Vec<ObjectId>,
    step: Step,
    /// The step's job, while it runs.
    going: Option<Going>,
    /// The automatic support profile the run sizes supports by, by name.
    profile: String,
    /// Settled polls the plate has been unsettled with nothing building, see [`IDLE_POLLS`].
    idle: u8,
    /// Steps landed, and the steps the run is reckoned to take in all.
    done: usize,
    steps: usize,
    shown: Highest,
    prepared: Vec<Prepared>,
}

impl Prepare {
    pub fn is_running(&self) -> bool {
        self.run.is_some()
    }

    /// The step under way and the share of the run done, counting only the steps it takes.
    pub fn progress(&self) -> Option<(Step, f32)> {
        let run = self.run.as_ref()?;
        let inner = run.going.as_ref().and_then(Going::fraction).unwrap_or(0.0);
        let share = (run.done as f32 + inner) / run.steps.max(1) as f32;
        Some((run.step, run.shown.keep(share.min(1.0))))
    }

    /// The step whose job is running now.
    #[cfg(test)]
    fn going(&self) -> Option<Step> {
        let run = self.run.as_ref()?;
        run.going.as_ref().map(|_| run.step)
    }

    /// What the last run decided for `object`, while it stands as that run left it.
    pub fn prepared(&self, object: &SceneObject) -> Option<&Prepared> {
        self.prepared
            .iter()
            .find(|prepared| prepared.still_on(object))
    }
}

/// Why a tool cannot start a job of its own now: Prepare is running its jobs on the plate.
pub fn holding(tools: &Tools) -> Option<String> {
    tools
        .prepare
        .is_running()
        .then(|| tr!("prepare-running-why"))
}

/// Why Prepare cannot start on the plate as it is, or `None` when it can: nothing to prepare,
/// or a job that will land on the plate still running.
pub fn blocker(doc: &Doc, tools: &Tools, machine: &Machine) -> Option<String> {
    if doc.scene.target_count() == 0 {
        return Some(tr!("prepare-nothing-visible"));
    }
    let busy = tools.prepare.is_running()
        || tools.orient.is_running()
        || tools.hollow.job.is_some()
        || tools.drain.job.is_some()
        || tools.drain.auto.is_some()
        || tools.drain.laying_plugs()
        || tools.supports.job.is_some()
        || tools.boolean.is_busy()
        || tools.cut.is_busy()
        || tools.relief.is_busy()
        || tools.text.is_busy()
        || doc.repairs.is_busy()
        || doc.imports.is_busy()
        || machine.slicing.job.is_some();
    busy.then(|| tr!("prepare-busy"))
}

/// Starts preparing what a tool would act on now: the picked models, or the whole plate.
pub fn start(doc: &mut Doc, tools: &mut Tools, machine: &mut Machine) -> Result<()> {
    if let Some(why) = blocker(doc, tools, machine) {
        bail!(why);
    }
    let ids: Vec<ObjectId> = doc
        .scene
        .targets()
        .filter(|object| !object.mesh.is_empty())
        .map(|object| object.id)
        .collect();
    if ids.is_empty() {
        bail!(tr!("prepare-nothing-visible"));
    }
    let Some(auto) = machine.slicing.catalogue.autos().next() else {
        bail!(tr!("supports-no-automatic"));
    };
    let profile = auto.profile.name.clone();
    let settings = ToolSettings::of(tools, &machine.slicing);
    let chosen = machine.slicing.chosen();
    doc.history.hold(&doc.scene, &settings, chosen);
    let mut run = Run {
        prepared: ids.iter().map(|id| Prepared::new(*id)).collect(),
        ids,
        step: Step::Orient,
        going: None,
        profile,
        idle: 0,
        done: 0,
        steps: 0,
        shown: Highest::default(),
    };
    run.steps = run.steps_left(&doc.scene);
    tools.prepare.run = Some(run);
    poll(doc, tools, machine, true);
    Ok(())
}

/// Stops the run between steps and inside the one going, and puts the plate back as it was.
pub fn cancel(doc: &mut Doc, tools: &mut Tools, machine: &mut Machine) {
    if tools.prepare.run.take().is_none() {
        return;
    }
    doc.history
        .abandon(&mut doc.scene, tools, &mut machine.slicing);
    machine.status = Status::Info(tr!("prepare-cancelled"));
}

/// Drops the run and its job without touching the plate, which is being replaced.
pub fn forget(tools: &mut Tools) {
    tools.prepare.run = None;
}

/// Moves the run on once the step in hand has landed. Called every frame after the tools'
/// own jobs are polled, `settled` with no button down; returns whether a run is still going.
pub fn poll(doc: &mut Doc, tools: &mut Tools, machine: &mut Machine, settled: bool) -> bool {
    let Some(mut run) = tools.prepare.run.take() else {
        return false;
    };
    // Another plate or project replaced the history: there is nothing left to prepare.
    if !doc.history.is_held() {
        return false;
    }
    if let Some(going) = run.going.as_mut() {
        let Some(ended) = going.poll(doc, tools) else {
            tools.prepare.run = Some(run);
            return true;
        };
        run.going = None;
        match ended {
            Ended::Landed(said) => machine.status = said,
            Ended::Stopped(why) => return stop(doc, tools, machine, why),
        }
        run.landed(&doc.scene);
        match run.step.next() {
            Some(next) => run.step = next,
            None => return finish(run, doc, tools, &mut machine.status),
        }
        run.steps = run.done + run.steps_left(&doc.scene);
    }
    if run.settling(doc, settled) {
        tools.prepare.run = Some(run);
        return true;
    }
    match advance(&mut run, doc, tools, machine) {
        Ok(true) => {
            tools.prepare.run = Some(run);
            true
        }
        Ok(false) => finish(run, doc, tools, &mut machine.status),
        Err(error) => stop(doc, tools, machine, Status::failed(&error)),
    }
}

/// Ends the run short: the plate as it was before it, and `why` said.
fn stop(doc: &mut Doc, tools: &mut Tools, machine: &mut Machine, why: Status) -> bool {
    doc.history
        .abandon(&mut doc.scene, tools, &mut machine.slicing);
    machine.status = why;
    false
}

/// Starts the first step from the one in hand that has models to work on. Answers whether
/// one started; none left is the run done.
fn advance(run: &mut Run, doc: &mut Doc, tools: &mut Tools, machine: &mut Machine) -> Result<bool> {
    loop {
        let chosen = run.wanting(run.step, &doc.scene);
        if !chosen.is_empty() {
            run.going = Some(Going::start(run.step, &chosen, doc, tools, machine)?);
            run.mark(&chosen, &doc.scene);
            return Ok(true);
        }
        match run.step.next() {
            Some(next) => run.step = next,
            None => return Ok(false),
        }
    }
}

/// Records the run as one edit and says what it did to the models still on the plate.
fn finish(run: Run, doc: &mut Doc, tools: &mut Tools, status: &mut Status) -> bool {
    doc.history.release();
    let count = doc.scene.named(&run.ids).count();
    tools.prepare.prepared = run.prepared;
    for prepared in &mut tools.prepare.prepared {
        if let Some(object) = doc.scene.get(prepared.id) {
            prepared.left_as(object);
        }
    }
    *status = Status::Info(tr!("prepare-done", count = count));
    false
}

/// Hollow's default wall where a sound, solid model saves enough resin to name.
fn worth_hollowing(object: &SceneObject) -> Option<Hollowing> {
    if object.hollow.is_hollow() {
        return None;
    }
    let found = &object.summary.diagnostics;
    hollowing(&object.mesh, object.transform.linear(), found)
}

fn unturned(object: &SceneObject) -> bool {
    object.transform.rotation.angle_between(Quat::IDENTITY) < UNTURNED_RAD
}

/// A model a step acts on, and the wall it is hollowed at where the step is Hollow.
type Chosen = (ObjectId, Option<Hollowing>);

impl Run {
    /// The models of the run `step` acts on, decided from the plate as it stands now.
    fn wanting(&self, step: Step, scene: &Scene) -> Vec<Chosen> {
        let chosen = |object: &SceneObject| {
            let wanted = match step {
                Step::Orient => unturned(object),
                Step::Repair => needs_repair(&object.summary.diagnostics),
                Step::Hollow => return worth_hollowing(object).map(|wall| (object.id, Some(wall))),
                Step::Drain => object.hollow.is_hollow(),
                Step::Cavity => object.supported_inside(),
                Step::Supports => true,
            };
            wanted.then_some((object.id, None))
        };
        scene.named(&self.ids).filter_map(chosen).collect()
    }

    /// How many steps from the one in hand the run is reckoned to take, as the plate stands:
    /// a model hollow, or worth hollowing, will be drained and held up inside.
    fn steps_left(&self, scene: &Scene) -> usize {
        let models: Vec<&SceneObject> = scene.named(&self.ids).collect();
        let any = |wanted: fn(&SceneObject) -> bool| models.iter().any(|object| wanted(object));
        let hollowed = any(|object| object.hollow.is_hollow() || worth_hollowing(object).is_some());
        Step::ALL[self.step.index()..]
            .iter()
            .filter(|step| match step {
                Step::Orient => any(unturned),
                Step::Repair => any(|object| needs_repair(&object.summary.diagnostics)),
                Step::Hollow => any(|object| worth_hollowing(object).is_some()),
                Step::Drain | Step::Cavity => hollowed,
                Step::Supports => true,
            })
            .count()
    }

    /// Whether the holes a step drilled are still being cut, or the surface supports stand
    /// on measured, which the next step works on. Only a settled frame counts as idle: the
    /// window starts no cutting while a button is down.
    fn settling(&mut self, doc: &Doc, settled: bool) -> bool {
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

    /// Notes the models `chosen` names as the step in hand found them.
    fn mark(&mut self, chosen: &[Chosen], scene: &Scene) {
        for (id, wall) in chosen {
            let prepared = self.prepared.iter_mut().find(|prepared| prepared.id == *id);
            if let Some((prepared, object)) = prepared.zip(scene.get(*id)) {
                prepared.started(self.step, object, *wall);
            }
        }
    }

    /// Notes what the step just landed changed on the plate.
    fn landed(&mut self, scene: &Scene) {
        self.done += 1;
        for prepared in &mut self.prepared {
            if let Some(object) = scene.get(prepared.id) {
                prepared.landed(self.step, object, &self.profile);
            }
        }
    }
}

#[cfg(test)]
mod tests;
