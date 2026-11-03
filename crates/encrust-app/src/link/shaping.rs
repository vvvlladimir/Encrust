//! The modelling, relief and mend of a recipe, as the server writes them, and the plan's
//! modelling steps they become.

use core_engine::project::{Axis, Keep};
use core_geometry::Vec3;
use core_joint::JointKind;
use core_volume::Boolean;

use std::path::PathBuf;

use core_geometry::Scalar;
use serde::Deserialize;

use super::recipe::Recipe;
use crate::prepare::{CutAsk, LabelAsk, Mend as MendAsk, ReliefAsk, Shaping, SplitAsk};

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Modelling {
    pub scale: Option<[Scalar; 3]>,
    pub combine: Option<Combine>,
    pub label: Option<Label>,
    pub cut: Option<Cut>,
    pub split_smallest_mm3: Option<Scalar>,
    pub part_off_at_mm: Option<[Scalar; 3]>,
    #[serde(default)]
    pub arrange: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Combine {
    pub op: Op,
    pub with: Vec<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Op {
    Union,
    Difference,
    Intersection,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Label {
    pub text: String,
    pub at_mm: [Scalar; 3],
    pub height_mm: Option<Scalar>,
    pub depth_mm: Option<Scalar>,
    #[serde(default)]
    pub sunk: bool,
    #[serde(default)]
    pub follow: bool,
    pub angle_deg: Option<Scalar>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Cut {
    pub axis: CutAxis,
    pub at_mm: Scalar,
    pub keep: Option<CutKeep>,
    pub joint: Option<Joint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CutAxis {
    X,
    Y,
    Z,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CutKeep {
    Both,
    Lower,
    Upper,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Joint {
    Dowels,
    Pins,
    Magnets,
    Key,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Relief {
    pub depth_mm: Scalar,
    pub image: Option<PathBuf>,
    pub tile_mm: Option<Scalar>,
    pub tile_angle_deg: Option<Scalar>,
    #[serde(default)]
    pub invert: bool,
    #[serde(default)]
    pub symmetric: bool,
    pub smooth_px: Option<Scalar>,
    pub hold_bottom_deg: Option<Scalar>,
    pub hold_top_deg: Option<Scalar>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Mend {
    #[serde(default)]
    pub flip_faces: Vec<u32>,
    #[serde(default)]
    pub drop_faces: Vec<u32>,
    #[serde(default)]
    pub drop_shells: Vec<usize>,
    /// Each `A-B:C-D`, by vertex numbers.
    #[serde(default)]
    pub bridge: Vec<String>,
}

impl Recipe {
    /// What the modelling steps do; a bridge that does not read is said in `unshown`.
    pub(super) fn shaping(&self, unshown: &mut Vec<String>) -> Shaping {
        let modelling = self.modelling.clone().unwrap_or_default();
        let split = match (modelling.part_off_at_mm, modelling.split_smallest_mm3) {
            (Some(at), _) => Some(SplitAsk::PartOff {
                at_mm: Vec3::from_array(at),
            }),
            (None, Some(smallest_mm3)) => Some(SplitAsk::Parts { smallest_mm3 }),
            (None, None) => None,
        };
        Shaping {
            mend: self.mend.as_ref().map(|mend| MendAsk {
                flip_faces: mend.flip_faces.clone(),
                bridges: mend
                    .bridge
                    .iter()
                    .filter_map(|edges| {
                        let read = bridge_of(edges);
                        if read.is_none() {
                            unshown.push(format!("bridge {edges}: not two edges A-B:C-D"));
                        }
                        read
                    })
                    .collect(),
                drop_faces: mend.drop_faces.clone(),
                drop_shells: mend.drop_shells.clone(),
            }),
            relief: self.relief.as_ref().map(|relief| ReliefAsk {
                depth_mm: relief.depth_mm,
                image: relief.image.clone(),
                tile_mm: relief.tile_mm,
                tile_angle_deg: relief.tile_angle_deg,
                invert: relief.invert,
                symmetric: relief.symmetric,
                smooth_px: relief.smooth_px,
                hold_bottom_deg: relief.hold_bottom_deg,
                hold_top_deg: relief.hold_top_deg,
            }),
            combine: modelling.combine.as_ref().map(|combine| match combine.op {
                Op::Union => Boolean::Union,
                Op::Difference => Boolean::Difference,
                Op::Intersection => Boolean::Intersection,
            }),
            label: modelling.label.map(|label| LabelAsk {
                text: label.text,
                at_mm: Vec3::from_array(label.at_mm),
                height_mm: label.height_mm,
                depth_mm: label.depth_mm,
                sunk: label.sunk,
                follow: label.follow,
                angle_deg: label.angle_deg,
            }),
            cut: modelling.cut.map(|cut| CutAsk {
                axis: match cut.axis {
                    CutAxis::X => Axis::X,
                    CutAxis::Y => Axis::Y,
                    CutAxis::Z => Axis::Z,
                },
                at_mm: cut.at_mm,
                keep: match cut.keep {
                    Some(CutKeep::Lower) => Keep::Below,
                    Some(CutKeep::Upper) => Keep::Above,
                    Some(CutKeep::Both) | None => Keep::Both,
                },
                joint: cut.joint.map(|joint| match joint {
                    Joint::Dowels => JointKind::Dowels,
                    Joint::Pins => JointKind::Pins,
                    Joint::Magnets => JointKind::Magnets,
                    Joint::Key => JointKind::Key,
                }),
            }),
            split,
            arrange: modelling.arrange,
        }
    }
}

/// `A-B:C-D`, the two open edges `--bridge` lays faces between.
fn bridge_of(text: &str) -> Option<([u32; 2], [u32; 2])> {
    let (a, b) = text.split_once(':')?;
    let edge = |text: &str| -> Option<[u32; 2]> {
        let (from, to) = text.split_once('-')?;
        Some([from.trim().parse().ok()?, to.trim().parse().ok()?])
    };
    Some((edge(a)?, edge(b)?))
}
