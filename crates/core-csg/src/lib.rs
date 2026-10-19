//! Exact union, difference and intersection of closed triangle meshes.
//!
//! A [`Solid`] is a mesh checked closed and readied once, so a chain of booleans pays for
//! the check and the index only at its start; see `docs/design/csg.md`.

mod error;
mod seams;
mod solid;

pub use error::CsgError;
pub use solid::Solid;
