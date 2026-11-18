use core_engine::project::{Axis, Keep, ObjectId};
use core_geometry::{Scalar, Vec3};
use core_joint::Joints;
use serde::{Deserialize, Serialize};

/// One action on a plate, naming the object it acts on; points and positions are plate
/// millimetres.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "edit", rename_all = "kebab-case")]
pub enum Edit {
    /// Cuts `object` across `axis` at `at_mm`, keeping `keep` and joining both halves by
    /// `joints`; with `lay_flat` each half lies on its cut face.
    Cut {
        object: ObjectId,
        axis: Axis,
        at_mm: Scalar,
        #[serde(default)]
        keep: Keep,
        #[serde(default)]
        joints: Joints,
        #[serde(default)]
        lay_flat: bool,
    },
    /// Moves `object` by `by_mm`.
    Move { object: ObjectId, by_mm: Vec3 },
    /// Sets the turn of `object` to `degrees` about X, then Y, then Z of the plate, about its
    /// centre of mass, and stands it on the plate.
    Rotate { object: ObjectId, degrees: Vec3 },
    /// Sets the scale of `object` to `factors` about its centre of mass and stands it on the
    /// plate.
    Scale { object: ObjectId, factors: Vec3 },
    /// Takes `object` off the plate, and the plugs laid for it.
    Delete { object: ObjectId },
    /// Puts a copy of `object`, with its supports, cavity and settings, `offset_mm` from it,
    /// right after it.
    Duplicate { object: ObjectId, offset_mm: Vec3 },
    /// Turns `object` inside out across `axis` of its own frame, where its bounds' centre
    /// stays.
    Mirror { object: ObjectId, axis: Axis },
    /// Breaks `object` into the bodies it is made of; a body under `smallest_mm3` (the
    /// default where unset) is joined to the one it touches, or dropped.
    Split {
        object: ObjectId,
        #[serde(default)]
        smallest_mm3: Option<Scalar>,
    },
    /// Cuts the part of `object` at plate point `at_mm` off where it joins the rest.
    PartOff { object: ObjectId, at_mm: Vec3 },
}

#[cfg(test)]
mod tests;
