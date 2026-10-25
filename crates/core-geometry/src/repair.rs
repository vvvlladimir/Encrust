//! The repairs that change a mesh only when they are asked to: closing the holes in it,
//! and dropping the faces laid over one another. See `docs/design/mesh-repair.md`.

mod duplicates;
mod flat;
mod holes;
mod triangulation;

pub use duplicates::remove_duplicate_faces;
pub use flat::collapse_flat_faces;
pub use holes::{Filled, fill_holes};

#[cfg(test)]
mod tests;
