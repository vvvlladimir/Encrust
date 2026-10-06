//! A sparse narrow-band signed distance field over a mesh, the CSG operators that combine
//! fields, and marching cubes back to a mesh.
//!
//! The field is the one engine behind hollowing, infill, drain holes and cuts; see
//! `docs/design/volume.md`, and `docs/design/hollowing.md` for what is built on it.

mod build;
mod csg;
mod drain;
mod error;
mod extract;
mod grid;
mod hollow;
mod infill;
mod model;
mod relief;
mod scatter;
mod sdf;
mod shells;
mod sign;
mod sweep;

pub use build::{FieldSettings, build};
pub use csg::{difference, intersection, offset, shell, union};
pub use drain::{
    CUT_WEIGHT, Channel, DrainHole, MOUTH_LIFT_MM, bores, channel_under, drill, hole_at, lift_for,
    pierce,
};
pub use error::VolumeError;
pub use extract::extract;
pub use grid::{TILE, VoxelGrid};
pub use hollow::{
    Blocker, HollowMode, HollowSettings, Hollowed, MIN_WALL_MM, hollow, hollow_at_scale,
    lattice_mm, sleeves,
};
pub use infill::{InfillPattern, InfillSettings};
pub use model::{HoleSize, ModelHollow, Shell, markers};
pub use relief::{Relief, ReliefSettings, press};
pub use sdf::Sdf;
pub use sign::SignMode;
