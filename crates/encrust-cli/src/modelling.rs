//! The flags of the window's Combine, Text, Cut and Split, which `stage/modelled.rs` folds to
//! edits of the plate after placing and before hollowing (ADR 0312).

use std::path::PathBuf;

use core_edit::Axis;
use core_geometry::{Scalar, Vec3};
use core_joint::JointKind;
use core_text::{LABEL_DEPTH_MM, LETTER_HEIGHT_MM};
use core_volume::PartSettings;

use crate::hollowing::point;

/// How `--combine` joins the models.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Op {
    Union,
    /// The first model less every other.
    Difference,
    Intersection,
}

impl From<Op> for core_edit::Op {
    fn from(op: Op) -> Self {
        match op {
            Op::Union => Self::Union,
            Op::Difference => Self::Difference,
            Op::Intersection => Self::Intersection,
        }
    }
}

/// Which halves `--cut` keeps.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Keep {
    #[default]
    Both,
    Lower,
    Upper,
}

impl From<Keep> for core_edit::Keep {
    fn from(keep: Keep) -> Self {
        match keep {
            Keep::Both => Self::Both,
            Keep::Lower => Self::Below,
            Keep::Upper => Self::Above,
        }
    }
}

/// What `--joint` joins the halves of a cut by.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Joint {
    Dowels,
    Pins,
    Magnets,
    Key,
}

impl From<Joint> for JointKind {
    fn from(joint: Joint) -> Self {
        match joint {
            Joint::Dowels => Self::Dowels,
            Joint::Pins => Self::Pins,
            Joint::Magnets => Self::Magnets,
            Joint::Key => Self::Key,
        }
    }
}

/// The modelling half of the command line.
#[derive(Debug, Clone, Default, clap::Args)]
#[command(next_help_heading = "Modelling")]
pub struct ModelArgs {
    /// Combine every model on the command line into one.
    #[arg(long, value_enum, value_name = "OP")]
    pub combine: Option<Op>,

    /// Write this text on the model, on the surface nearest --label-at.
    #[arg(long, value_name = "TEXT", requires = "label_at")]
    pub label: Option<String>,

    /// The point on the plate, millimetres, the label is written nearest to.
    #[arg(long, value_name = "X,Y,Z", value_parser = point, allow_hyphen_values = true,
          requires = "label")]
    pub label_at: Option<Vec3>,

    /// How tall the label's capitals stand, millimetres.
    #[arg(long, value_name = "MM", default_value_t = LETTER_HEIGHT_MM, requires = "label")]
    pub label_height: Scalar,

    /// How far the letters stand out of the surface, or into it, millimetres.
    #[arg(long, value_name = "MM", default_value_t = LABEL_DEPTH_MM, requires = "label")]
    pub label_depth: Scalar,

    /// Sink the letters into the surface rather than raise them out of it.
    #[arg(long, requires = "label")]
    pub label_sunk: bool,

    /// Bend every letter onto the surface under it rather than stand it flat.
    #[arg(long, requires = "label")]
    pub label_follow: bool,

    /// How far the line of text is turned about the surface's normal, degrees anticlockwise.
    #[arg(
        long,
        value_name = "DEG",
        default_value_t = 0.0,
        allow_hyphen_values = true,
        requires = "label"
    )]
    pub label_angle: Scalar,

    /// A font file to write the label in, in place of the bundled IBM Plex Sans.
    #[arg(long, value_name = "PATH", requires = "label")]
    pub label_font: Option<PathBuf>,

    /// Cut the model with a plane square to an axis at a plate coordinate, millimetres:
    /// `z:10` is 10 mm over the plate.
    #[arg(long, value_name = "AXIS:MM", value_parser = cut_at, allow_hyphen_values = true)]
    pub cut: Option<(Axis, Scalar)>,

    /// Which halves of the cut are kept.
    #[arg(long, value_enum, default_value_t = Keep::Both, requires = "cut")]
    pub keep: Keep,

    /// Join the two halves of the cut, at the window's default sizes.
    #[arg(long, value_enum, requires = "cut")]
    pub joint: Option<Joint>,

    /// Break each model into the bodies it is made of.
    #[arg(long, conflicts_with = "part_off")]
    pub split: bool,

    /// A piece holding less than this is a scrap, not a part of its own, cubic millimetres.
    #[arg(long, value_name = "MM3", default_value_t = PartSettings::default().smallest_mm3,
          requires = "split")]
    pub split_smallest: Scalar,

    /// Cut off the part of the model this point on the plate is on, flat where it narrows
    /// into the rest.
    #[arg(long, value_name = "X,Y,Z", value_parser = point, allow_hyphen_values = true)]
    pub part_off: Option<Vec3>,
}

impl ModelArgs {
    /// Whether anything is asked of the models.
    pub fn wanted(&self) -> bool {
        self.combine.is_some()
            || self.label.is_some()
            || self.cut.is_some()
            || self.split
            || self.part_off.is_some()
    }

    /// Whether a model may come out as several pieces, each to be put down on its own.
    pub fn splits(&self) -> bool {
        self.cut.is_some() || self.split || self.part_off.is_some()
    }
}

/// `axis:mm` as the axis of the plate a cut is square to and the coordinate it crosses at.
fn cut_at(text: &str) -> Result<(Axis, Scalar), String> {
    let (axis, at) = text
        .split_once(':')
        .ok_or_else(|| format!("a cut is AXIS:MM, such as z:10, got {text:?}"))?;
    let axis = match axis.trim().to_ascii_lowercase().as_str() {
        "x" => Axis::X,
        "y" => Axis::Y,
        "z" => Axis::Z,
        other => return Err(format!("a cut is across x, y or z, got {other:?}")),
    };
    let at: Scalar = at.trim().parse().map_err(|error| format!("{error}"))?;
    if !at.is_finite() {
        return Err(format!("{text:?} names no plane"));
    }
    Ok((axis, at))
}
