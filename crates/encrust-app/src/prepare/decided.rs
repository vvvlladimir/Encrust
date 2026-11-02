use std::sync::Arc;

use core_engine::Hollowing;
use core_geometry::Transform;

use super::Step;
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
    /// The model as the run left it, so a later edit retires what is said of it.
    left: Option<(usize, Transform)>,
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
            left: None,
        }
    }

    /// Notes `step` started on the model, with the wall a Hollow is asked at.
    pub(super) fn started(&mut self, step: Step, wall: Option<Hollowing>) {
        match step {
            Step::Orient => self.oriented = true,
            Step::Repair => self.repaired = true,
            Step::Hollow => self.hollowed = wall,
            Step::Drain => self.drained = true,
            Step::Supports | Step::Cavity => {}
        }
    }

    /// Notes what `step` left on `object` once it landed: a hollow that did not come, or
    /// the supports it stands on.
    pub(super) fn landed(&mut self, step: Step, object: &SceneObject, profile: &str) {
        match step {
            Step::Hollow if !object.hollow.is_hollow() => self.hollowed = None,
            Step::Supports if !object.supports.is_empty() => {
                self.supports = Some(profile.to_owned());
            }
            _ => {}
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
