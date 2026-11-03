use std::sync::Arc;

use core_engine::Hollowing;
use core_geometry::{Quat, Transform};

use super::{Step, UNTURNED_RAD};
use crate::scene::{ObjectId, SceneObject};

/// What a run did to one model, as the command line's `prepared` states it.
#[derive(Debug, Clone, PartialEq)]
pub struct Prepared {
    pub id: ObjectId,
    pub oriented: bool,
    pub repaired: bool,
    /// The wall it was hollowed at and the resin that saves; `None` printed solid.
    pub hollowed: Option<Hollowing>,
    pub drained: bool,
    /// The automatic profile its supports were sized by, by name.
    pub supports: Option<String>,
    /// The model as the step in hand found it, which what it landed is measured against.
    was: Option<Was>,
    /// The model as the run left it, so a later edit retires what is said of it.
    left: Option<(usize, Transform)>,
}

/// What a step may change on a model, as it stood when the step started.
#[derive(Debug, Clone, PartialEq)]
struct Was {
    step: Step,
    mesh: usize,
    rotation: Quat,
    holes: usize,
    /// The wall a Hollow was asked at.
    wall: Option<Hollowing>,
}

impl Prepared {
    pub(super) fn new(id: ObjectId) -> Self {
        Self {
            id,
            oriented: false,
            repaired: false,
            hollowed: None,
            drained: false,
            supports: None,
            was: None,
            left: None,
        }
    }

    /// Notes `object` as `step` found it, with the wall a Hollow is asked at.
    pub(super) fn started(&mut self, step: Step, object: &SceneObject, wall: Option<Hollowing>) {
        self.was = Some(Was {
            step,
            mesh: Arc::as_ptr(&object.mesh) as usize,
            rotation: object.transform.rotation,
            holes: object.hollow.drains().len(),
            wall,
        });
    }

    /// Decides from what `step` changed on `object` once it landed what the run did to it.
    pub(super) fn landed(&mut self, step: Step, object: &SceneObject, profile: &str) {
        let Some(was) = self.was.take().filter(|was| was.step == step) else {
            return;
        };
        match step {
            Step::Orient => {
                self.oriented =
                    object.transform.rotation.angle_between(was.rotation) > UNTURNED_RAD;
            }
            Step::Repair => self.repaired = Arc::as_ptr(&object.mesh) as usize != was.mesh,
            Step::Hollow => self.hollowed = was.wall.filter(|_| object.hollow.is_hollow()),
            Step::Drill | Step::Drain => {
                self.drained |= object.hollow.drains().len() > was.holes;
            }
            Step::Supports => {
                self.supports = (!object.supports.is_empty()).then(|| profile.to_owned());
            }
            Step::Cavity => {}
        }
    }

    pub(super) fn left_as(&mut self, object: &SceneObject) {
        self.left = Some(key(object));
    }

    /// Whether `object` stands as the run left it.
    pub(super) fn still_on(&self, object: &SceneObject) -> bool {
        object.id == self.id && self.left == Some(key(object))
    }
}

fn key(object: &SceneObject) -> (usize, Transform) {
    (Arc::as_ptr(object.uncut()) as usize, object.transform)
}
