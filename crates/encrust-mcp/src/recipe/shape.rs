//! What a job does to the model's shape before it is prepared — repair's finer settings, its
//! size, the modelling the command line does and a relief — each field one or more flags.

use std::path::PathBuf;

use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};

use super::{kebab, triple};

/// Repair's finer settings, and the faces and shells to change by hand by the numbers
/// `inspect_model` gives.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Mend {
    /// How far a vertex may stand off an open edge to be stitched in, mm, as `--seam-tolerance`.
    pub seam_tolerance_mm: Option<f32>,
    /// Smallest area a shell enclosing nothing keeps, mm², as `--smallest-shell`.
    pub smallest_shell_mm2: Option<f32>,
    /// Faces to turn the other way, numbered from 0, as `--flip-faces`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flip_faces: Vec<u32>,
    /// Faces to drop, as `--drop-faces`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub drop_faces: Vec<u32>,
    /// Shells to drop, numbered from 1 as `inspect_model` lists them, as `--drop-shells`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub drop_shells: Vec<u32>,
    /// Faces laid between two open edges, each `A-B:C-D` by vertex numbers, as `--bridge`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bridge: Vec<String>,
}

/// How several models are joined into one, as `--combine`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Op {
    Union,
    /// The job's model less every other.
    Difference,
    Intersection,
}

/// Which halves of a cut stay, as `--keep`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Keep {
    Both,
    Lower,
    Upper,
}

/// What joins the halves of a cut, as `--joint`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Joint {
    Dowels,
    Pins,
    Magnets,
    Key,
}

/// An axis a cut is square to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Axis {
    X,
    Y,
    Z,
}

/// The modelling the command line does before it prepares a model.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Modelling {
    /// Uniform factor, or X, Y and Z factors, as `--scale`.
    pub scale: Option<[f32; 3]>,
    pub combine: Option<Combine>,
    pub label: Option<Label>,
    pub cut: Option<Cut>,
    /// Breaks the model into the bodies it is made of, as `--split`; a piece holding less
    /// than this many mm³ is a scrap, joined back.
    pub split_smallest_mm3: Option<f32>,
    /// Cuts off the part of the model this plate point is on, flat where it narrows into the
    /// rest, as `--part-off`.
    pub part_off_at_mm: Option<[f32; 3]>,
    /// Spreads the pieces over the plate, biggest first, as `--arrange`: what a cut, a split
    /// or several models need so none stands on another.
    #[serde(default)]
    pub arrange: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Combine {
    pub op: Op,
    /// Absolute paths of the other models, joined with the job's.
    pub with: Vec<PathBuf>,
}

/// Text written on the model, as `--label` and its flags.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Label {
    pub text: String,
    /// The plate point the label is written nearest to, mm.
    pub at_mm: [f32; 3],
    /// How tall the capitals stand, mm.
    pub height_mm: Option<f32>,
    /// How far the letters stand out of the surface, or into it, mm.
    pub depth_mm: Option<f32>,
    /// Sinks the letters into the surface.
    #[serde(default)]
    pub sunk: bool,
    /// Bends every letter onto the surface under it.
    #[serde(default)]
    pub follow: bool,
    /// Turns the line about the surface's normal, degrees anticlockwise.
    pub angle_deg: Option<f32>,
}

/// A plane cut square to an axis, as `--cut`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Cut {
    pub axis: Axis,
    /// The plate coordinate along the axis, mm.
    pub at_mm: f32,
    pub keep: Option<Keep>,
    pub joint: Option<Joint>,
}

/// A texture pressed into the model, as `--relief` and its flags.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Relief {
    /// How deep, mm; negative sinks it.
    pub depth_mm: f32,
    /// Absolute path of a picture tiled over the model from three sides, for a model with no
    /// texture of its own.
    pub image: Option<PathBuf>,
    /// How wide one tile of `image` is, mm.
    pub tile_mm: Option<f32>,
    pub tile_angle_deg: Option<f32>,
    /// Reads black as high.
    #[serde(default)]
    pub invert: bool,
    /// Reads mid grey as the surface, so the model keeps about its size.
    #[serde(default)]
    pub symmetric: bool,
    /// Blurs the texture by this many of its pixels first.
    pub smooth_px: Option<f32>,
    /// Leaves faces within this many degrees of facing the plate as they are.
    pub hold_bottom_deg: Option<f32>,
    /// Leaves faces within this many degrees of facing up as they are.
    pub hold_top_deg: Option<f32>,
}

impl Mend {
    pub(super) fn push_flags(&self, flags: &mut Vec<String>) {
        if let Some(mm) = self.seam_tolerance_mm {
            flags.push(format!("--seam-tolerance={mm}"));
        }
        if let Some(mm2) = self.smallest_shell_mm2 {
            flags.push(format!("--smallest-shell={mm2}"));
        }
        for (flag, numbers) in [
            ("--flip-faces", &self.flip_faces),
            ("--drop-faces", &self.drop_faces),
            ("--drop-shells", &self.drop_shells),
        ] {
            if !numbers.is_empty() {
                let listed: Vec<String> = numbers.iter().map(u32::to_string).collect();
                flags.push(format!("{flag}={}", listed.join(",")));
            }
        }
        flags.extend(self.bridge.iter().map(|edges| format!("--bridge={edges}")));
    }
}

impl Modelling {
    /// The models joined to the job's, which follow it on the command line.
    pub(super) fn others(&self) -> impl Iterator<Item = &PathBuf> {
        self.combine.iter().flat_map(|combine| &combine.with)
    }

    pub(super) fn push_flags(&self, flags: &mut Vec<String>) {
        if let Some(scale) = self.scale {
            flags.push(format!("--scale={}", triple(scale)));
        }
        if let Some(combine) = &self.combine {
            flags.push(format!("--combine={}", kebab(&combine.op)));
        }
        if let Some(label) = &self.label {
            label.push_flags(flags);
        }
        if let Some(cut) = &self.cut {
            flags.push(format!("--cut={}:{}", kebab(&cut.axis), cut.at_mm));
            if let Some(keep) = cut.keep {
                flags.push(format!("--keep={}", kebab(&keep)));
            }
            if let Some(joint) = cut.joint {
                flags.push(format!("--joint={}", kebab(&joint)));
            }
        }
        if let Some(smallest) = self.split_smallest_mm3 {
            flags.extend(["--split".into(), format!("--split-smallest={smallest}")]);
        }
        if let Some(at) = self.part_off_at_mm {
            flags.push(format!("--part-off={}", triple(at)));
        }
        if self.arrange {
            flags.push("--arrange".into());
        }
    }
}

impl Label {
    fn push_flags(&self, flags: &mut Vec<String>) {
        flags.push(format!("--label={}", self.text));
        flags.push(format!("--label-at={}", triple(self.at_mm)));
        if let Some(mm) = self.height_mm {
            flags.push(format!("--label-height={mm}"));
        }
        if let Some(mm) = self.depth_mm {
            flags.push(format!("--label-depth={mm}"));
        }
        if self.sunk {
            flags.push("--label-sunk".into());
        }
        if self.follow {
            flags.push("--label-follow".into());
        }
        if let Some(deg) = self.angle_deg {
            flags.push(format!("--label-angle={deg}"));
        }
    }
}

impl Relief {
    pub(super) fn push_flags(&self, flags: &mut Vec<String>) {
        flags.push(format!("--relief={}", self.depth_mm));
        if let Some(image) = &self.image {
            flags.push(format!("--relief-image={}", image.display()));
        }
        for (flag, value) in [
            ("--relief-tile", self.tile_mm),
            ("--relief-angle", self.tile_angle_deg),
            ("--relief-smooth", self.smooth_px),
            ("--relief-hold-bottom", self.hold_bottom_deg),
            ("--relief-hold-top", self.hold_top_deg),
        ] {
            if let Some(value) = value {
                flags.push(format!("{flag}={value}"));
            }
        }
        if self.invert {
            flags.push("--relief-invert".into());
        }
        if self.symmetric {
            flags.push("--relief-symmetric".into());
        }
    }
}
