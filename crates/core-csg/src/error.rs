use thiserror::Error;

/// What can go wrong reading a mesh as a solid or combining two.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CsgError {
    /// The mesh does not close a volume: it is open, crosses itself round an edge, or the
    /// engine refused it for its own reason, which the text gives.
    #[error("the mesh does not enclose a solid: {0}")]
    NotSolid(String),

    /// The two solids were sound and the engine still could not combine them.
    #[error("the boolean could not be completed: {0}")]
    Failed(String),
}
