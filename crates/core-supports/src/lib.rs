//! Supports: where a column stands under a model, how tips merge into trees, and the mesh
//! they are printed as; see `docs/design/supports.md`.

mod base;
mod branch;
mod buttress;
mod clear;
mod clearances;
mod column;
mod config;
mod detect;
mod edit;
mod field;
#[cfg(test)]
mod fixtures;
mod generate;
mod group;
mod inline;
mod islands;
mod load;
mod lost;
mod low;
mod mesher;
mod model;
mod need;
mod pad;
mod pick;
mod placed;
mod preview;
mod project;
mod raft;
mod region;
mod sample;
mod shaded;
mod step;
mod trapped;
mod tree;

pub use base::{BraceBand, BraceLevel, SKATE_BEVEL, SKATE_LENGTH, SKATE_WIDTH, brace_levels};
pub use branch::grow;
pub use clearances::Clearances;
pub use column::{Column, Landing, SupportPoint, columns, landing, neck_of};
pub use edit::{
    Carried, Refused, allowed, carried, carried_held, eased, fits, followed, on_model, standing_on,
    tip_carried, with_riders,
};
pub use generate::generate_supports;
pub use group::Profiles;
pub use inline::{INLINE_SPACING_MM, inline};
pub use lost::{Lost, Unstood, lost};
pub use low::low_points;
pub use mesher::{FOOT_TAPER, foot_bevel_mm, mesh_alone, mesh_groups, mesh_trees};
pub use model::{Grown, Growth, ModelSupports};
pub use need::{Need, Reason};
pub use pad::{PAD_SECTORS, PadCut};
pub use pick::{Grab, Part, Slack, grab, grab_along, support_under};
pub use placed::Placed;
pub use preview::{Preview, preview, stands};
pub use project::{InfillPattern, ProjectSettings, Projected, project};
pub use raft::{GRID_BAR_MM, GRID_HOLE_MM, LIP_WIDTH_MM};
pub use region::{Blocked, Shown, WallSide};
pub use shaded::{CREASE_DEG, Shaded, shaded};
pub use trapped::{TrapScan, Trapped};
pub use tree::{SupportTree, TreeNode};
