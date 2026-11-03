//! What a run does at each step: decide as `--prepare` does, do what it was told, or leave the
//! step out. Prepare for print decides every step; a job a chat client edits says some.

use core_engine::project::{DrainState, HollowState};
use core_engine::{Hollowing, PREPARE_ORIENT, PREPARE_REPAIR, hollowing, needs_repair};
use core_geometry::{Quat, Scalar, Vec3};
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

/// How a model is turned: by the search, or by a turn given outright.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Orient {
    Search {
        preset: OrientPreset,
        /// How far from the way it came in the search may turn it, degrees.
        max_tilt_deg: Option<Scalar>,
    },
    /// From the way the model came in, as `--rotate` turns it.
    Turn(Quat),
}

/// The holes a run drills: at points, and where the engine proves they drain.
#[derive(Debug, Clone, PartialEq)]
pub struct Drains {
    /// Plate millimetres as the model stands when they are drilled.
    pub at_mm: Vec<Vec3>,
    pub auto: bool,
    /// With `auto`, a vent high on each pocket too.
    pub vent: bool,
    /// The size every hole is drilled to, the Drain tool's where `None`.
    pub size: Option<DrainState>,
}

/// What a run stands a model on.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Stand {
    /// Built to this support profile, or sized by the automatic one where `None`.
    pub profile: Option<String>,
    /// Supports stood under the surface nearest each, plate millimetres, in the group in hand.
    pub at_mm: Vec<Vec3>,
    /// Stands only those.
    pub only: bool,
}

/// What a run does at each step.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    /// X, Y and Z factors the model is scaled by from the way it came in, before anything else.
    pub scale: Option<Vec3>,
    pub orient: Choice<Orient>,
    pub repair: Choice<RepairMethod>,
    pub hollow: Choice<HollowState>,
    /// Plate points the wall is kept solid round, at the hollow's blocker radius.
    pub keep_solid: Vec<Vec3>,
    /// `None` drills nothing.
    pub drains: Option<Drains>,
    /// `None` stands nothing on supports.
    pub supports: Option<Stand>,
    pub cavity: bool,
}

/// A model a step acts on, and the wall it is hollowed at where the step is Hollow.
pub(super) type Chosen = (ObjectId, Option<Hollowing>);

impl Plan {
    /// Every step decided: Prepare for print.
    pub fn autopilot() -> Self {
        Self {
            scale: None,
            orient: Choice::Decide,
            repair: Choice::Decide,
            hollow: Choice::Decide,
            keep_solid: Vec::new(),
            drains: Some(Drains {
                at_mm: Vec::new(),
                auto: true,
                vent: true,
                size: None,
            }),
            supports: Some(Stand::default()),
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
            Step::Drill => self
                .drains
                .as_ref()
                .is_some_and(|drains| !drains.at_mm.is_empty()),
            Step::Drain => {
                self.drains.as_ref().is_some_and(|drains| drains.auto) && object.hollow.is_hollow()
            }
            Step::Supports => self.supports.is_some(),
            Step::Cavity => self.cavity && object.supported_inside(),
        };
        chosen.filter(|_| wanted)
    }

    /// Whether `step` is reckoned to act on `object` once the steps before it have: a model
    /// hollow, or to be hollowed, will be drained and held up inside.
    pub(super) fn reckons(&self, step: Step, object: &SceneObject) -> bool {
        let hollowed = object.hollow.is_hollow() || self.wall(object).is_some();
        match step {
            Step::Drain => self.drains.as_ref().is_some_and(|drains| drains.auto) && hollowed,
            Step::Cavity => self.cavity && hollowed,
            step => self.wants(step, object).is_some(),
        }
    }

    /// How Orient turns a model.
    pub(super) fn orient(&self) -> Orient {
        match self.orient {
            Choice::Given(orient) => orient,
            Choice::Decide | Choice::Skip => Orient::Search {
                preset: PREPARE_ORIENT,
                max_tilt_deg: None,
            },
        }
    }

    /// The repair Repair runs.
    pub(super) fn method(&self) -> RepairMethod {
        match self.repair {
            Choice::Given(method) => method,
            Choice::Decide | Choice::Skip => PREPARE_REPAIR,
        }
    }

    /// The Hollow settings given, where they were.
    pub(super) fn hollow_given(&self) -> Option<&HollowState> {
        match &self.hollow {
            Choice::Given(state) => Some(state),
            Choice::Decide | Choice::Skip => None,
        }
    }

    /// The wall a solid `object` is hollowed at: the one given, or Hollow's default where it
    /// saves enough resin to name.
    fn wall(&self, object: &SceneObject) -> Option<Hollowing> {
        if object.hollow.is_hollow() {
            return None;
        }
        match &self.hollow {
            Choice::Decide => {
                let found = &object.summary.diagnostics;
                hollowing(&object.mesh, object.transform.linear(), found)
            }
            Choice::Given(state) => Some(Hollowing {
                wall_mm: state.thickness_mm,
                saves_mm3: 0.0,
            }),
            Choice::Skip => None,
        }
    }
}

fn unturned(object: &SceneObject) -> bool {
    object.transform.rotation.angle_between(Quat::IDENTITY) < UNTURNED_RAD
}
