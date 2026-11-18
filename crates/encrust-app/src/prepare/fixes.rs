//! Check's Fix all: the built-in recipe every automatic answer is a step of, run on the
//! whole plate whatever is picked (ADR 0416).

use anyhow::Result;
use core_recipe::built_in;

use super::work::{Batch, Work};
use super::{Run, start_on};
use crate::check::IssueKind;
use crate::scene::ObjectId;
use crate::state::{Doc, Machine, Tools};
use crate::status::Status;

/// The built-in recipe Fix all takes.
const FIX_RECIPE: &str = "fix-issues";

/// Starts every automatic answer on the models of the plate being edited.
pub fn start_fixes(doc: &mut Doc, tools: &mut Tools, machine: &mut Machine) -> Result<()> {
    let plate = doc.scene.active_plate();
    let ids: Vec<ObjectId> = doc
        .scene
        .printable(plate)
        .filter(|object| !object.mesh.is_empty())
        .map(|object| object.id)
        .collect();
    start_on(built_in(FIX_RECIPE)?, ids, doc, tools, machine)
}

impl Run {
    /// Notes what a step said of `kind`; a retry's word does not replace the first answer's.
    pub(super) fn answered(&mut self, kind: IssueKind, said: &Status) {
        if !self.answers.iter().any(|(held, _)| *held == kind) {
            self.answers.push((kind, said.clone()));
        }
    }

    /// Notes what a landed drain did for the resin the models kept.
    pub(super) fn note_landing(&mut self, batch: &Batch, said: &Status) {
        if matches!(batch.work, Work::DrainAuto { .. }) {
            self.answered(IssueKind::KeepsResin, said);
        }
    }

    /// Hands Check what the run answered, as the edit its hold is about to record.
    pub(super) fn hand_over_answers(&self, doc: &Doc, tools: &mut Tools) {
        let changed = doc.history.has_unrecorded(&doc.scene);
        for (kind, said) in &self.answers {
            tools.check.answers.note(&doc.history, changed, *kind, said);
        }
    }
}
