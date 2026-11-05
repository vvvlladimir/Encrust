//! Prepare for print: a recipe's steps taken on the picked models, each by starting a tool's
//! own job and waiting for it to land, all one undo (ADR 0402, 0403).

use std::collections::VecDeque;

use anyhow::{Result, bail};
use core_geometry::Quat;
use core_recipe::{Metrics, Plan, StepKind};

use crate::i18n::tr;
use crate::job::Highest;
use crate::scene::{ObjectId, SceneObject};
use crate::state::{Doc, Machine, Tools};
use crate::status::Status;
use crate::supports::BuiltTo;
use crate::tool_settings::ToolSettings;

mod check;
mod decided;
mod foresee;
mod measures;
mod recipes;
mod run;
mod save;
mod step;
mod work;

pub use decided::Prepared;
pub use foresee::{Foresight, PlanLine, foreseen_lines, running_lines};
pub use recipes::{Library, kind_name, note_text};
use run::Next;
use step::{Ended, Going};
use work::Batch;

/// Settled polls the plate may wait on cuts or a support surface no job is building, which the
/// window starts a frame after a landing, before a step starts anyway: a failed one never restarts.
const IDLE_POLLS: u8 = 3;

/// A turn this small, radians, is a model nobody turned: the window's `--rotate` left out.
const UNTURNED_RAD: f32 = 1e-4;

/// The recipe Prepare for print runs until another is picked.
pub const DEFAULT_RECIPE: &str = "prepare";

/// The job a step runs, which the bar names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Orient,
    Repair,
    Hollow,
    Drain,
    Supports,
    /// Supports inside a cavity, after those outside.
    Cavity,
    /// Drain's Check drainage.
    Scan,
    Check,
    Arrange,
    Save,
    /// Waiting for Continue.
    Pause,
}

impl Step {
    /// What the bar says while it runs.
    pub fn label(self) -> String {
        match self {
            Self::Orient => tr!("prepare-step-orient"),
            Self::Repair => tr!("prepare-step-repair"),
            Self::Hollow => tr!("prepare-step-hollow"),
            Self::Drain => tr!("prepare-step-drain"),
            Self::Supports => tr!("prepare-step-supports"),
            Self::Cavity => tr!("prepare-step-cavity"),
            Self::Scan => tr!("prepare-step-scan"),
            Self::Check => tr!("prepare-step-check"),
            Self::Arrange => tr!("prepare-step-arrange"),
            Self::Save => tr!("prepare-step-save"),
            Self::Pause => tr!("prepare-step-pause"),
        }
    }
}

/// The recipe picked, the run going, if any, and what the last one decided.
#[derive(Debug)]
pub struct Prepare {
    /// The recipe a run takes, by id: a built-in one or the user's (ADR 0405).
    pub recipe: String,
    run: Option<Run>,
    prepared: Vec<Prepared>,
    /// The plan of the picked recipe for the models in hand, before a run.
    pub foresight: Foresight,
    /// The recipes there are to pick from.
    pub library: Library,
}

impl Default for Prepare {
    fn default() -> Self {
        Self {
            recipe: DEFAULT_RECIPE.to_owned(),
            run: None,
            prepared: Vec::new(),
            foresight: Foresight::default(),
            library: Library::default(),
        }
    }
}

/// One run: the models it took, the plan, the step in hand and what has been decided so far.
#[derive(Debug)]
struct Run {
    ids: Vec<ObjectId>,
    plan: Plan,
    /// What a pause says while the run waits for Continue.
    paused: Option<String>,
    /// The plan step decided next, and the one in hand, by place.
    next: usize,
    index: usize,
    /// The batches of the step in hand not yet started.
    queue: VecDeque<Batch>,
    /// The batch whose job is running, with its job.
    going: Option<(Batch, Going)>,
    /// The job the bar names: the one running, or the last.
    step: Step,
    /// What the run measured on each model after a step, as `trapped` after a drain.
    measured: Vec<(ObjectId, Metrics)>,
    /// Where each plan step stands, which the plan list shows while the run goes.
    marks: Vec<crate::ui::PlanMark>,
    /// The models a supports step of this run stood on supports, which a later one replaces.
    supported: Vec<ObjectId>,
    /// What each model's latest supports step built to, which a check's retry stands islands on.
    built: Vec<(ObjectId, BuiltTo)>,
    /// The models whose latest drain step left their holes with no plug.
    unplugged: Vec<ObjectId>,
    /// Settled polls the plate has been unsettled with nothing building, see [`IDLE_POLLS`].
    idle: u8,
    /// Steps done, and the steps the run is reckoned to take in all.
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
        let inner = run.going.as_ref().and_then(|(_, going)| going.fraction());
        let share = (run.done as f32 + inner.unwrap_or(0.0)) / run.steps.max(1) as f32;
        Some((run.step, run.shown.keep(share.min(1.0))))
    }

    /// The plan the run takes and where each of its steps stands, while it runs.
    pub fn marks(&self) -> Option<(&Plan, &[crate::ui::PlanMark])> {
        let run = self.run.as_ref()?;
        Some((&run.plan, &run.marks))
    }

    /// What the pause the run waits at says, while it waits.
    pub fn paused(&self) -> Option<&str> {
        self.run.as_ref()?.paused.as_deref()
    }

    /// Moves a paused run on to its next step.
    pub fn resume(&mut self) {
        if let Some(run) = &mut self.run {
            run.paused = None;
        }
    }

    /// The step whose job is running now.
    #[cfg(test)]
    fn going(&self) -> Option<Step> {
        let run = self.run.as_ref()?;
        run.going.as_ref().map(|(batch, _)| batch.work.step())
    }

    /// How many times the check whose stack is being measured has supported islands.
    #[cfg(test)]
    fn attempt(&self) -> Option<u8> {
        let run = self.run.as_ref()?;
        match run.going.as_ref()?.0.work {
            work::Work::Check { attempt } => Some(attempt),
            _ => None,
        }
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
    if busy {
        return Some(tr!("prepare-busy"));
    }
    let catalogue = &machine.slicing.catalogue;
    let prepare = &tools.prepare;
    prepare
        .library
        .plan(catalogue, &prepare.recipe)
        .err()
        .map(|error| format!("{error:#}"))
}

/// Starts preparing what a tool would act on now, the picked models or the whole plate, by
/// the recipe picked.
pub fn start(doc: &mut Doc, tools: &mut Tools, machine: &mut Machine) -> Result<()> {
    let catalogue = &machine.slicing.catalogue;
    let plan = tools
        .prepare
        .library
        .plan(catalogue, &tools.prepare.recipe)?;
    start_with(plan, doc, tools, machine)
}

/// The same by `plan`.
fn start_with(plan: Plan, doc: &mut Doc, tools: &mut Tools, machine: &mut Machine) -> Result<()> {
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
    let mut steps = plan.recipe().steps.iter();
    let supports = steps.any(|step| step.enabled && step.kind == StepKind::Supports);
    if supports && machine.slicing.catalogue.autos().next().is_none() {
        bail!(tr!("supports-no-automatic"));
    }
    let settings = ToolSettings::of(tools, &machine.slicing);
    let chosen = machine.slicing.chosen();
    doc.history.hold(&doc.scene, &settings, chosen);
    tools.prepare.run = Some(Run::new(plan, ids));
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
    // Nothing to poll while paused: the frame after Continue moves it on.
    if run.paused.is_some() {
        tools.prepare.run = Some(run);
        return false;
    }
    if let Some(going) = poll_going(&mut run, doc, tools, machine) {
        if going {
            tools.prepare.run = Some(run);
        }
        return going;
    }
    run.leave_unplugged(doc, tools, machine);
    if run.settling(doc, settled) {
        tools.prepare.run = Some(run);
        return true;
    }
    let next = run.advance(doc, tools, machine);
    run.leave_unplugged(doc, tools, machine);
    match next {
        Ok(Next::Working) => {
            tools.prepare.run = Some(run);
            true
        }
        Ok(Next::Done) => finish(run, doc, tools, &mut machine.status),
        Ok(Next::Stopped(why)) => stop(doc, tools, machine, why),
        Err(error) => stop(doc, tools, machine, Status::failed(&error)),
    }
}

/// Lands the step's job once it has ended. Answers whether the run still goes when the
/// frame ends here: its job still running, or the run stopped; `None` to move it on.
fn poll_going(
    run: &mut Run,
    doc: &mut Doc,
    tools: &mut Tools,
    machine: &mut Machine,
) -> Option<bool> {
    let (batch, mut going) = run.going.take()?;
    let Some(ended) = going.poll(doc, tools, machine) else {
        run.going = Some((batch, going));
        return Some(true);
    };
    match ended {
        Ended::Landed(said) => {
            machine.status = said;
            if let Err(error) = run.checked(doc, tools, machine, &batch) {
                return Some(stop(doc, tools, machine, Status::failed(&error)));
            }
            run.landed(&doc.scene, batch, going.built());
        }
        Ended::Stopped(why) if run.skips() => {
            machine.status = why;
            run.queue.clear();
        }
        Ended::Stopped(why) => return Some(stop(doc, tools, machine, why)),
    }
    run.batch_ended();
    None
}

/// Ends the run short: the plate as it was before it, and `why` said.
fn stop(doc: &mut Doc, tools: &mut Tools, machine: &mut Machine, why: Status) -> bool {
    doc.history
        .abandon(&mut doc.scene, tools, &mut machine.slicing);
    machine.status = why;
    false
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

fn unturned(object: &SceneObject) -> bool {
    object.transform.rotation.angle_between(Quat::IDENTITY) < UNTURNED_RAD
}

#[cfg(test)]
mod tests;
