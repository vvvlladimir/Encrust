use core_csg::CsgError;
use core_geometry::Scalar;
use thiserror::Error;

/// Why a cut's halves could not be joined.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum JointError {
    /// A size the kind asked for is not a length it can be built at; the name says which.
    #[error("the {what} must be a positive length, got {value_mm} mm")]
    BadSize {
        what: &'static str,
        value_mm: Scalar,
    },

    /// A half, or the model it was cut from, is not a solid the boolean can read, or the
    /// boolean failed on it.
    #[error("the joint could not be built into the halves: {0}")]
    Boolean(#[from] CsgError),
}
