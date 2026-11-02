//! Prepare for print: the decisions `--prepare` makes, taken on the picked models by starting
//! each tool's own job in turn and waiting for it to land, all one undo (ADR 0397).

use anyhow::{Result, bail};
use core_engine::{Hollowing, PREPARE_ORIENT, PREPARE_REPAIR, hollowing, needs_repair};
use core_geometry::Quat;

use crate::i18n::tr;
use crate::repair::RepairState;
use crate::scene::{ObjectId, Scene, SceneObject};
use crate::state::{Doc, Machine, Tools};
use crate::status::Status;
use crate::supports::Side;

mod decided;

pub use decided::Prepared;

/// Polls the plate may wait on cuts or a support surface no job is building, which the window
/// starts a frame after a landing, before a step starts anyway: a failed one never restarts.
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
    /// The step's job was started and has not landed yet.
    going: bool,
    /// The automatic support profile the run sizes supports by, by name.
    profile: String,
    /// Polls the plate has been unsettled with nothing building, see [`IDLE_POLLS`].
    idle: u8,
    prepared: Vec<Prepared>,
}

impl Prepare {
    pub fn is_running(&self) -> bool {
        self.run.is_some()
    }

    /// The step under way and the share of the run done.
    pub fn progress(&self) -> Option<(Step, f32)> {
        let run = self.run.as_ref()?;
        let share = run.step.index() as f32 / Step::ALL.len() as f32;
        Some((run.step, share))
    }

    /// What the last run decided for `object`, while it stands as that run left it.
    pub fn prepared(&self, object: &SceneObject) -> Option<&Prepared> {
        self.prepared
            .iter()
            .find(|prepared| prepared.still_on(object))
    }
}

/// Why Prepare cannot start on the plate as it is, or `None` when it can.
pub fn blocker(doc: &Doc, tools: &Tools) -> Option<String> {
    if doc.scene.target_count() == 0 {
        return Some(tr!("prepare-nothing-visible"));
    }
    let busy = tools.prepare.is_running()
        || tools.orient.is_running()
        || tools.hollow.job.is_some()
        || tools.drain.auto.is_some()
        || tools.supports.job.is_some()
        || doc.repairs.is_busy();
    busy.then(|| tr!("prepare-busy"))
}

/// Starts preparing what a tool would act on now: the picked models, or the whole plate.
pub fn start(doc: &mut Doc, tools: &mut Tools, machine: &mut Machine) -> Result<()> {
    if let Some(why) = blocker(doc, tools) {
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
    doc.history.hold();
    tools.prepare.run = Some(Run {
        prepared: ids.iter().map(|id| Prepared::new(*id)).collect(),
        ids,
        step: Step::Orient,
        going: false,
        profile,
        idle: 0,
    });
    poll(doc, tools, machine);
    Ok(())
}

/// Stops the run between steps and inside the one going, and puts the plate back as it was.
pub fn cancel(doc: &mut Doc, tools: &mut Tools, status: &mut Status) {
    if tools.prepare.run.take().is_none() {
        return;
    }
    stop_jobs(doc, tools);
    doc.history.abandon(&mut doc.scene, tools);
    *status = Status::Info(tr!("prepare-cancelled"));
}

/// Moves the run on once the step in hand has landed. Called every frame after the tools'
/// own jobs are polled; returns whether a run is still going.
pub fn poll(doc: &mut Doc, tools: &mut Tools, machine: &mut Machine) -> bool {
    let Some(mut run) = tools.prepare.run.take() else {
        return false;
    };
    // Another plate or project replaced the history: there is nothing left to prepare.
    if !doc.history.is_held() {
        stop_jobs(doc, tools);
        return false;
    }
    if run.going {
        if step_busy(run.step, doc, tools) {
            tools.prepare.run = Some(run);
            return true;
        }
        run.going = false;
        // A repair that leaves holes is said and the run goes on, as the command line's does.
        let failed = run.step != Step::Repair && matches!(machine.status, Status::Error(_));
        if failed {
            doc.history.abandon(&mut doc.scene, tools);
            return false;
        }
        run.landed(&doc.scene);
        match run.step.next() {
            Some(next) => run.step = next,
            None => return finish(run, doc, tools, &mut machine.status),
        }
    }
    if run.settling(doc) {
        tools.prepare.run = Some(run);
        return true;
    }
    match advance(&mut run, doc, tools, machine) {
        Ok(true) => {
            tools.prepare.run = Some(run);
            true
        }
        Ok(false) => finish(run, doc, tools, &mut machine.status),
        Err(error) => {
            doc.history.abandon(&mut doc.scene, tools);
            machine.status = Status::failed(&error);
            false
        }
    }
}

/// Starts the first step from the one in hand that has models to work on. Answers whether
/// one started; none left is the run done.
fn advance(run: &mut Run, doc: &mut Doc, tools: &mut Tools, machine: &mut Machine) -> Result<bool> {
    loop {
        let chosen = run.wanting(run.step, &doc.scene);
        if !chosen.is_empty() {
            start_step(run.step, &chosen, doc, tools, machine)?;
            run.mark(run.step, &chosen);
            run.going = true;
            return Ok(true);
        }
        match run.step.next() {
            Some(next) => run.step = next,
            None => return Ok(false),
        }
    }
}

fn start_step(
    step: Step,
    chosen: &[Chosen],
    doc: &mut Doc,
    tools: &mut Tools,
    machine: &mut Machine,
) -> Result<()> {
    let ids: Vec<ObjectId> = chosen.iter().map(|(id, _)| *id).collect();
    let ids = ids.as_slice();
    let slicing = &machine.slicing;
    let layer_mm = slicing.layer_height_mm();
    let scene = &doc.scene;
    match step {
        Step::Orient => {
            let plans = tools.supports.automatic_plans(&slicing.catalogue)?;
            let proof = slicing
                .printer
                .as_ref()
                .map(|printer| (printer, &slicing.material));
            tools
                .orient
                .start_on(scene, ids, (PREPARE_ORIENT, plans), proof)?;
        }
        Step::Repair => {
            let state = RepairState {
                method: PREPARE_REPAIR,
                ..RepairState::default()
            };
            for id in ids {
                doc.repairs.start(scene, *id, state, &mut machine.status);
            }
        }
        Step::Hollow => {
            let walls: Vec<(ObjectId, f32)> = chosen
                .iter()
                .filter_map(|(id, wall)| Some((*id, wall.as_ref()?.wall_mm)))
                .collect();
            tools.hollow.start_walls(scene, &walls)?;
        }
        Step::Drain => tools.drain.start_auto_on(scene, ids, layer_mm)?,
        Step::Supports | Step::Cavity => {
            let side = match step {
                Step::Cavity => Side::Inside,
                _ => Side::Outside,
            };
            let asked = (layer_mm, &slicing.catalogue);
            tools.supports.start_automatic_on(scene, ids, asked, side)?;
        }
    }
    Ok(())
}

/// Whether the job `step` started is still running.
fn step_busy(step: Step, doc: &Doc, tools: &Tools) -> bool {
    match step {
        Step::Orient => tools.orient.is_running(),
        Step::Repair => doc.repairs.is_busy(),
        Step::Hollow => tools.hollow.job.is_some(),
        Step::Drain => tools.drain.auto.is_some(),
        Step::Supports | Step::Cavity => tools.supports.job.is_some(),
    }
}

/// Gives up every job a step may have going; none of them lands.
fn stop_jobs(doc: &mut Doc, tools: &mut Tools) {
    let mut quiet = Status::default();
    tools.orient.cancel(&mut quiet);
    doc.repairs.cancel(&mut quiet);
    tools.hollow.job = None;
    tools.drain.cancel_auto(&mut quiet);
    tools.supports.job = None;
}

/// Records the run as one edit and says what it did.
fn finish(run: Run, doc: &mut Doc, tools: &mut Tools, status: &mut Status) -> bool {
    doc.history.release();
    let count = run.ids.len();
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

/// A model a step acts on, and the wall it is hollowed at where the step is Hollow.
type Chosen = (ObjectId, Option<Hollowing>);

impl Run {
    /// The models of the run `step` acts on, decided from the plate as it stands now.
    fn wanting(&self, step: Step, scene: &Scene) -> Vec<Chosen> {
        let chosen = |object: &SceneObject| {
            let wanted = match step {
                Step::Orient => {
                    object.transform.rotation.angle_between(Quat::IDENTITY) < UNTURNED_RAD
                }
                Step::Repair => needs_repair(&object.summary.diagnostics),
                Step::Hollow => return worth_hollowing(object).map(|wall| (object.id, Some(wall))),
                Step::Drain => object.hollow.is_hollow(),
                Step::Cavity => object.supported_inside(),
                Step::Supports => true,
            };
            wanted.then_some((object.id, None))
        };
        self.ids
            .iter()
            .filter_map(|id| scene.get(*id).and_then(chosen))
            .collect()
    }

    /// Whether the holes a step drilled are still being cut, or the surface supports stand
    /// on measured, which the next step works on.
    fn settling(&mut self, doc: &Doc) -> bool {
        let unsettled = self
            .ids
            .iter()
            .filter_map(|id| doc.scene.get(*id))
            .any(|object| {
                object.hollow.wants_drilling(object.uncut())
                    || object.unbuilt_support_surface().is_some()
            });
        if !unsettled || doc.drilling.is_running() || doc.surfaces.is_running() {
            self.idle = 0;
            return unsettled;
        }
        self.idle += 1;
        self.idle < IDLE_POLLS
    }

    /// Notes that `step` was started on the models `chosen` names.
    fn mark(&mut self, step: Step, chosen: &[Chosen]) {
        for (id, wall) in chosen {
            if let Some(prepared) = self.prepared.iter_mut().find(|prepared| prepared.id == *id) {
                prepared.started(step, *wall);
            }
        }
    }

    /// Notes what the step just landed left on the plate.
    fn landed(&mut self, scene: &Scene) {
        for prepared in &mut self.prepared {
            if let Some(object) = scene.get(prepared.id) {
                prepared.landed(self.step, object, &self.profile);
            }
        }
    }
}

#[cfg(test)]
mod tests;
