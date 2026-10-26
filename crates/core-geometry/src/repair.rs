//! The repairs that change a mesh only when they are asked to: closing the holes in it,
//! stitching its seams and dropping the faces laid over one another or the specks apart.
//! See `docs/design/mesh-repair.md`.

mod duplicates;
mod edit;
mod flat;
mod holes;
mod seams;
mod shells;
mod triangulation;

pub use duplicates::remove_duplicate_faces;
pub use edit::{BridgeError, bridge, flip_faces, remove_faces};
pub use flat::collapse_flat_faces;
pub(crate) use holes::boundary_loops;
pub use holes::{Filled, fill_holes, fill_holes_within};
pub use seams::{DEFAULT_SEAM_TOLERANCE, Stitched, stitch_seams};
pub use shells::{
    DEFAULT_SMALLEST_SHELL_MM2, Shell, drop_small_shells, list_shells, remove_shells,
};

#[cfg(test)]
mod tests;
