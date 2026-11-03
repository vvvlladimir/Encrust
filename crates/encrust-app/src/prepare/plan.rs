//! What a run does at each step: decide as `--prepare` does, do what it was told, or leave the
//! step out. Prepare for print decides every step; a job a chat client edits says some.

use core_engine::{Hollowing, PREPARE_ORIENT, PREPARE_REPAIR, hollowing, needs_repair};
use core_geometry::{Quat, Scalar};
use core_plate::OrientPreset;
use core_volume::RepairMethod;

use super::{Step, UNTURNED_RAD};
use crate::scene::{ObjectId, SceneObject};

/// One step's part of a plan.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Choice<T> {
    /// Done where the autopilot would do it, as it would.
    Decide,
    /// Done on every model, as given.
    Given(T),
    Skip,
}

/// What a run does at each step. Supports, drains and the cavity's supports have nothing to
/// give: where they are not skipped they are done where they can be, as the autopilot does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plan {
    pub orient: Choice<OrientPreset>,
    pub repair: Choice<RepairMethod>,
    /// The wall, millimetres.
    pub hollow: Choice<Scalar>,
    pub drain: bool,
    pub supports: bool,
    pub cavity: bool,
}

/// A model a step acts on, and the wall it is hollowed at where the step is Hollow.
pub(super) type Chosen = (ObjectId, Option<Hollowing>);

impl Plan {
    /// Every step decided: Prepare for print.
    pub fn autopilot() -> Self {
        Self {
            orient: Choice::Decide,
            repair: Choice::Decide,
            hollow: Choice::Decide,
            drain: true,
            supports: true,
            cavity: true,
        }
    }

    /// Whether `step` acts on `object`, and the wall it hollows it at.
    pub(super) fn wants(&self, step: Step, object: &SceneObject) -> Option<Chosen> {
        let chosen = Some((object.id, None));
        let wanted = match step {
            Step::Orient => match self.orient {
                Choice::Decide => unturned(object),
                Choice::Given(_) => true,
                Choice::Skip => false,
            },
            Step::Repair => match self.repair {
                Choice::Decide => needs_repair(&object.summary.diagnostics),
                Choice::Given(_) => true,
                Choice::Skip => false,
            },
            Step::Hollow => return self.wall(object).map(|wall| (object.id, Some(wall))),
            Step::Drain => self.drain && object.hollow.is_hollow(),
            Step::Supports => self.supports,
            Step::Cavity => self.cavity && object.supported_inside(),
        };
        chosen.filter(|_| wanted)
    }

    /// Whether `step` is reckoned to act on `object` once the steps before it have: a model
    /// hollow, or to be hollowed, will be drained and held up inside.
    pub(super) fn reckons(&self, step: Step, object: &SceneObject) -> bool {
        let hollowed = object.hollow.is_hollow() || self.wall(object).is_some();
        match step {
            Step::Drain => self.drain && hollowed,
            Step::Cavity => self.cavity && hollowed,
            step => self.wants(step, object).is_some(),
        }
    }

    /// The search Orient runs.
    pub(super) fn preset(&self) -> OrientPreset {
        match self.orient {
            Choice::Given(preset) => preset,
            Choice::Decide | Choice::Skip => PREPARE_ORIENT,
        }
    }

    /// The repair Repair runs.
    pub(super) fn method(&self) -> RepairMethod {
        match self.repair {
            Choice::Given(method) => method,
            Choice::Decide | Choice::Skip => PREPARE_REPAIR,
        }
    }

    /// The wall a solid `object` is hollowed at: the one given, or Hollow's default where it
    /// saves enough resin to name.
    fn wall(&self, object: &SceneObject) -> Option<Hollowing> {
        if object.hollow.is_hollow() {
            return None;
        }
        match self.hollow {
            Choice::Decide => {
                let found = &object.summary.diagnostics;
                hollowing(&object.mesh, object.transform.linear(), found)
            }
            Choice::Given(wall_mm) => Some(Hollowing {
                wall_mm,
                saves_mm3: 0.0,
            }),
            Choice::Skip => None,
        }
    }
}

fn unturned(object: &SceneObject) -> bool {
    object.transform.rotation.angle_between(Quat::IDENTITY) < UNTURNED_RAD
}
