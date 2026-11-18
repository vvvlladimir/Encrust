use core_engine::project::{Axis, Keep, ObjectId};
use core_geometry::Scalar;
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
}

#[cfg(test)]
mod tests;
