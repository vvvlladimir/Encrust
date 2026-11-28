//! Starting a run: why one cannot, and on which models it starts.

use anyhow::{Result, bail};
use core_recipe::{Plan, StepKind};

use super::{Run, poll};
use crate::i18n::tr;
use crate::scene::ObjectId;
use crate::state::{Doc, Machine, Tools};
use crate::tool_settings::ToolSettings;

/// Why Prepare cannot start on the plate as it is, or `None` when it can: nothing to prepare,
/// or a job that will land on the plate still running.
pub fn blocker(doc: &Doc, tools: &Tools, machine: &Machine) -> Option<String> {
    if let Some(why) = held_up(doc, tools, machine) {
        return Some(why);
    }
    let catalogue = &machine.slicing.catalogue;
    let prepare = &tools.prepare;
    prepare
        .library
        .plan(catalogue, &prepare.recipe)
        .err()
        .map(|error| format!("{error:#}"))
}

/// Why no run can start on the plate as it is: nothing on it, or a job still to land.
pub fn held_up(doc: &Doc, tools: &Tools, machine: &Machine) -> Option<String> {
    if doc.scene.target_count() == 0 {
        return Some(tr!("prepare-nothing-visible"));
    }
    (tools.prepare.is_running() || busy(doc, tools, machine)).then(|| tr!("prepare-busy"))
}

/// Whether a job that will land on the plate is running.
pub fn busy(doc: &Doc, tools: &Tools, machine: &Machine) -> bool {
    tool_busy(doc, tools) || doc.imports.is_busy() || machine.slicing.job.is_some()
}

/// Whether a tool's job that will change a model on the plate is running.
pub fn tool_busy(doc: &Doc, tools: &Tools) -> bool {
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
        || doc.repairs.hand.is_busy()
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
pub(super) fn start_with(
    plan: Plan,
    doc: &mut Doc,
    tools: &mut Tools,
    machine: &mut Machine,
) -> Result<()> {
    let ids: Vec<ObjectId> = doc
        .scene
        .targets()
        .filter(|object| !object.mesh.is_empty())
        .map(|object| object.id)
        .collect();
    start_on(plan, ids, doc, tools, machine)
}

/// The same by `plan`, on the models `ids`.
pub(super) fn start_on(
    plan: Plan,
    ids: Vec<ObjectId>,
    doc: &mut Doc,
    tools: &mut Tools,
    machine: &mut Machine,
) -> Result<()> {
    can_start(&plan, &ids, doc, tools, machine)?;
    hold(doc, tools, machine);
    tools.prepare.run = Some(Run::new(plan, ids));
    poll(doc, tools, machine, true);
    Ok(())
}

/// Why `plan` cannot start on `ids` now.
fn can_start(
    plan: &Plan,
    ids: &[ObjectId],
    doc: &Doc,
    tools: &Tools,
    machine: &Machine,
) -> Result<()> {
    if let Some(why) = held_up(doc, tools, machine) {
        bail!(why);
    }
    if ids.is_empty() {
        bail!(tr!("prepare-nothing-visible"));
    }
    let mut steps = plan.recipe().steps.iter();
    let supports = steps.any(|step| step.enabled && step.kind == StepKind::Supports);
    if supports && machine.slicing.catalogue.autos().next().is_none() {
        bail!(tr!("supports-no-automatic"));
    }
    Ok(())
}

/// Opens the run's one undo entry, the plate and the tools as they stand.
fn hold(doc: &mut Doc, tools: &Tools, machine: &Machine) {
    let settings = ToolSettings::of(tools, &machine.slicing);
    let chosen = machine.slicing.chosen();
    doc.history.hold(&doc.scene, &settings, chosen);
}
