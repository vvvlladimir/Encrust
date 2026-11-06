//! Prepare for print: a recipe's steps taken on the picked models, each by starting a tool's
//! own job and waiting for it to land, one undo until it waits on the user (ADR 0402, 0403,
//! 0407).

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
mod measures;
mod recipes;
mod run;
mod save;
mod step;
mod work;

pub use recipes::{Library, kind_glyph, kind_name};
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

/// The recipe picked and the run going, if any.
#[derive(Debug)]
pub struct Prepare {
    /// The recipe a run takes, by id: a built-in one or the user's (ADR 0405).
    pub recipe: String,
    run: Option<Run>,
    /// The recipes there are to pick from.
    pub library: Library,
}

impl Default for Prepare {
    fn default() -> Self {
        Self {
            recipe: DEFAULT_RECIPE.to_owned(),
            run: None,
            library: Library::default(),
        }
    }
}

/// Why a run waits: the plate is the user's meanwhile, its undo too, until Continue takes
/// it again or the run is ended as it stands (ADR 0407).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wait {
    /// What the pause says, or why the step failed.
    pub why: String,
    /// A step failed, which Continue tries again; else a pause, which Continue passes.
    pub failed: bool,
    /// The recipe's step it waits at, counted from 1.
    pub step: usize,
}

/// One run: the models it took, the plan, the step in hand and what it measured so far.
#[derive(Debug)]
struct Run {
    ids: Vec<ObjectId>,
    plan: Plan,
    /// Why the run waits for Continue: a pause or a failed step (ADR 0407).
    waiting: Option<Wait>,
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
}

impl Prepare {
    pub fn is_running(&self) -> bool {
        self.run.is_some()
    }

    /// Whether a run holds the plate: one going, not one waiting for Continue.
    pub fn is_holding(&self) -> bool {
        self.run.as_ref().is_some_and(|run| run.waiting.is_none())
    }

    /// Why the run waits for Continue, while it does.
    pub fn waiting(&self) -> Option<&Wait> {
        self.run.as_ref()?.waiting.as_ref()
    }

    /// The step under way and the share of the run done, counting only the steps it takes.
    pub fn progress(&self) -> Option<(Step, f32)> {
        let run = self.run.as_ref()?;
        let inner = run.going.as_ref().and_then(|(_, going)| going.fraction());
        let share = (run.done as f32 + inner.unwrap_or(0.0)) / run.steps.max(1) as f32;
        Some((run.step, run.shown.keep(share.min(1.0))))
    }

    /// The plan the run takes, while it runs.
    #[cfg(test)]
    fn plan(&self) -> Option<&Plan> {
        self.run.as_ref().map(|run| &run.plan)
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
}

/// Why a tool cannot start a job of its own now: Prepare is running its jobs on the plate.
pub fn holding(tools: &Tools) -> Option<String> {
    tools
        .prepare
        .is_holding()
        .then(|| tr!("prepare-running-why"))
}

/// Why Prepare cannot start on the plate as it is, or `None` when it can: nothing to prepare,
/// or a job that will land on the plate still running.
pub fn blocker(doc: &Doc, tools: &Tools, machine: &Machine) -> Option<String> {
    if doc.scene.target_count() == 0 {
        return Some(tr!("prepare-nothing-visible"));
    }
    if tools.prepare.is_running() || busy(doc, tools, machine) {
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

/// Whether a job that will land on the plate is running.
fn busy(doc: &Doc, tools: &Tools, machine: &Machine) -> bool {
    tools.orient.is_running()
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
        || machine.slicing.job.is_some()
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

/// Moves a waiting run on, on the plate as the user left it: past its pause, or trying the
/// failed step again; a job of a tool's own still running refuses it.
pub fn resume(doc: &mut Doc, tools: &mut Tools, machine: &mut Machine) -> Result<()> {
    if tools.prepare.waiting().is_none() {
        return Ok(());
    }
    if busy(doc, tools, machine) {
        bail!(tr!("prepare-busy"));
    }
    let settings = ToolSettings::of(tools, &machine.slicing);
    let chosen = machine.slicing.chosen();
    doc.history.hold(&doc.scene, &settings, chosen);
    if let Some(run) = &mut tools.prepare.run {
        run.waiting = None;
        run.measure_again();
    }
    machine.status = Status::Idle;
    poll(doc, tools, machine, true);
    Ok(())
}

/// Ends a waiting run, the plate kept as it stands: what the run did before it waited is one
/// undo.
pub fn end_waiting(tools: &mut Tools, status: &mut Status) {
    if tools.prepare.waiting().is_some() {
        tools.prepare.run = None;
        *status = Status::Info(tr!("prepare-ended"));
    }
}

/// Stops the run between steps and inside the one going, and puts the plate back as it was
/// when the run, or its last Continue, took it; a waiting run is ended as it stands.
pub fn cancel(doc: &mut Doc, tools: &mut Tools, machine: &mut Machine) {
    if tools.prepare.waiting().is_some() {
        end_waiting(tools, &mut machine.status);
        return;
    }
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
    // Waiting, the plate is the user's and the history free until Continue.
    if run.waiting.is_some() {
        tools.prepare.run = Some(run);
        return false;
    }
    // Another plate or project replaced the history: there is nothing left to prepare.
    if !doc.history.is_held() {
        return false;
    }
    if let Some(going) = poll_going(&mut run, doc, tools, machine) {
        if going || run.waiting.is_some() {
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
        Ok(Next::Working) if run.waiting.is_some() => {
            // A pause: what the run did so far is one edit, the plate the user's.
            doc.history.release();
            tools.prepare.run = Some(run);
            false
        }
        Ok(Next::Working) => {
            tools.prepare.run = Some(run);
            true
        }
        Ok(Next::Done) => finish(&run, doc, &mut machine.status),
        Ok(Next::Stopped(why)) => halt(run, doc, tools, machine, why),
        Err(error) => halt(run, doc, tools, machine, Status::failed(&error)),
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
                run.queue.push_front(batch);
                return Some(halted(run, doc, machine, Status::failed(&error)));
            }
            run.landed(&doc.scene, batch, going.built());
        }
        Ended::Stopped(why) if run.skips() => {
            machine.status = why;
            run.queue.clear();
        }
        Ended::Stopped(why) => {
            run.queue.push_front(batch);
            return Some(halted(run, doc, machine, why));
        }
    }
    run.batch_ended();
    None
}

/// Halts the run at the step that failed or the gate that holds, `why` said, and keeps it
/// waiting.
fn halt(
    mut run: Run,
    doc: &mut Doc,
    tools: &mut Tools,
    machine: &mut Machine,
    why: Status,
) -> bool {
    halted(&mut run, doc, machine, why);
    tools.prepare.run = Some(run);
    false
}

/// Halts `run` where it stands: what it did so far recorded as one edit, the plate the
/// user's until Continue tries the failed step again. `why` is said by the row, not the
/// stage's notice. Answers that no job runs.
fn halted(run: &mut Run, doc: &mut Doc, machine: &mut Machine, why: Status) -> bool {
    run.halt(why.text());
    doc.history.release();
    machine.status = Status::Idle;
    false
}

/// Records the run as one edit and says how many models it prepared.
fn finish(run: &Run, doc: &mut Doc, status: &mut Status) -> bool {
    doc.history.release();
    let count = doc.scene.named(&run.ids).count();
    *status = Status::Info(tr!("prepare-done", count = count));
    false
}

fn unturned(object: &SceneObject) -> bool {
    object.transform.rotation.angle_between(Quat::IDENTITY) < UNTURNED_RAD
}

#[cfg(test)]
mod tests;
