//! A sparse narrow-band signed distance field over a mesh, the CSG operators that combine
//! fields, and marching cubes back to a mesh.
//!
//! The field is the one engine behind hollowing, infill, drain holes and cuts; see
//! `docs/design/volume.md`, and `docs/design/hollowing.md` for what is built on it.

mod boolean;
mod build;
mod cancel;
mod csg;
mod drain;
mod error;
mod extract;
mod grid;
mod hollow;
mod infill;
mod model;
mod parts;
mod relief;
mod scatter;
mod sdf;
mod shells;
mod sign;
mod stack;
mod sweep;

pub use boolean::{Boolean, Combined, combine, combine_exact};
pub use build::{FieldSettings, build};
pub use cancel::Cancel;
pub use csg::{difference, intersection, offset, shell, union};
pub use drain::{
    CUT_WEIGHT, Channel, DrainHole, MOUTH_LIFT_MM, channel_under, drill, hole_at, lift_for, pierce,
};
pub use error::VolumeError;
pub use extract::extract;
pub use grid::{TILE, VoxelGrid};
pub use hollow::{
    Blocker, CavityField, HollowMode, HollowSettings, Hollowed, MIN_WALL_MM, carve, cavity_field,
    hollow, hollow_at_scale, lattice_mm, sleeves,
};
pub use infill::{InfillPattern, InfillSettings};
pub use model::{HoleSize, ModelHollow, PENDING_MARKER_MM, Shell, markers};
pub use parts::{PartSettings, PartedOff, Parts, part_off, parts};
pub use relief::{
    AngleMask, Relief, ReliefBasis, ReliefSettings, Tiling, prepared, press, press_reusing,
    press_tiled, press_tiled_reusing,
};
pub use sdf::Sdf;
pub use sign::SignMode;
pub use stack::{Panel, StackField};
