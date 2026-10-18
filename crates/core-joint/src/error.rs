use core_csg::CsgError;
use thiserror::Error;

/// Why a cut's halves could not be joined.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum JointError {
    /// A half, or the model it was cut from, is not a solid the boolean can read, or the
    /// boolean failed on it.
    #[error("the joint could not be built into the halves: {0}")]
    Boolean(#[from] CsgError),
}
