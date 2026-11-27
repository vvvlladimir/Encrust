//! The window's Position moves the other placing flags leave out: a mirror, a move by a
//! distance and copies, each an edit `stage/placed.rs` makes (ADR 0459).

use core_edit::Axis;
use core_geometry::Vec3;

use crate::hollowing::point;

/// Mirror, move and copies of one model.
#[derive(Debug, Clone, Default, clap::Args)]
pub struct PlacingArgs {
    /// Mirror the model across this axis of its own, where its middle stays.
    #[arg(long, value_enum, value_name = "AXIS")]
    pub mirror: Option<MirrorAxis>,

    /// Move the model by this much once placed, millimetres along X, Y and Z.
    #[arg(long = "move", value_name = "X,Y,Z", value_parser = point, allow_hyphen_values = true)]
    pub move_by: Option<Vec3>,

    /// Print this many more of the model, each a copy of it as prepared; --arrange spreads
    /// them over the plate.
    #[arg(long, value_name = "N")]
    pub copies: Option<u32>,
}

/// An axis `--mirror` turns a model across.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum MirrorAxis {
    X,
    Y,
    Z,
}

impl From<MirrorAxis> for Axis {
    fn from(axis: MirrorAxis) -> Self {
        match axis {
            MirrorAxis::X => Self::X,
            MirrorAxis::Y => Self::Y,
            MirrorAxis::Z => Self::Z,
        }
    }
}
