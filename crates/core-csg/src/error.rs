use thiserror::Error;

/// What can go wrong reading a mesh as a solid or combining two.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CsgError {
    /// The mesh is not one closed, two-sided surface: it is open, branches at an edge or
    /// has no faces worth a solid. The text is the engine's own reason.
    #[error("the mesh does not enclose a solid: {0}")]
    NotSolid(String),

    /// The two solids were sound and the engine still could not combine them.
    #[error("the boolean could not be completed: {0}")]
    Failed(String),
}
