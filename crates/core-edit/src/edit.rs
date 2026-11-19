use core_engine::project::{Axis, HollowState, Keep, ObjectId};
use core_geometry::{Scalar, Vec3};
use core_joint::Joints;
use core_volume::{HoleKind, HoleSize, ReliefSettings, RepairMethod, Tiling};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// One action on a plate, naming the object it acts on; points and positions are plate
/// millimetres.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "edit", rename_all = "kebab-case")]
pub enum Edit {
    /// Cuts `object` across `axis` at `at_mm`, keeping `keep` and joining both halves by
    /// `joints`; with `lay_flat` each half lies on its cut face.
    Cut {
        object: ObjectId,
        axis: Axis,
        at_mm: Scalar,
        #[serde(default)]
        keep: Keep,
        #[serde(default)]
        joints: Joints,
        #[serde(default)]
        lay_flat: bool,
    },
    /// Moves `object` by `by_mm`.
    Move { object: ObjectId, by_mm: Vec3 },
    /// Sets the turn of `object` to `degrees` about X, then Y, then Z of the plate, about its
    /// centre of mass, and stands it on the plate.
    Rotate { object: ObjectId, degrees: Vec3 },
    /// Sets the scale of `object` to `factors` about its centre of mass and stands it on the
    /// plate.
    Scale { object: ObjectId, factors: Vec3 },
    /// Takes `object` off the plate, and the plugs laid for it.
    Delete { object: ObjectId },
    /// Puts a copy of `object`, with its supports, cavity and settings, `offset_mm` from it,
    /// right after it.
    Duplicate { object: ObjectId, offset_mm: Vec3 },
    /// Turns `object` inside out across `axis` of its own frame, where its bounds' centre
    /// stays.
    Mirror { object: ObjectId, axis: Axis },
    /// Breaks `object` into the bodies it is made of; a body under `smallest_mm3` (the
    /// default where unset) is joined to the one it touches, or dropped.
    Split {
        object: ObjectId,
        #[serde(default)]
        smallest_mm3: Option<Scalar>,
    },
    /// Cuts the part of `object` at plate point `at_mm` off where it joins the rest.
    PartOff { object: ObjectId, at_mm: Vec3 },
    /// Replaces `objects` by the one `op` makes of them, the first being what a difference
    /// keeps. With `toward` set the cut is exact and takes only the side of the first the
    /// others stand on it from; unset, it is exact where every object is a solid and sampled
    /// where one is not.
    Combine {
        objects: Vec<ObjectId>,
        op: Op,
        #[serde(default)]
        toward: Option<Vec3>,
    },
    /// Writes `text` on `object` at the surface nearest plate point `at_mm`, its capitals
    /// `height_mm` tall and its letters `depth_mm` out of the surface or, unless `raised`, into
    /// it; unset, the size is the window's Text tool's. With `follow_surface` each letter bends
    /// onto the surface under it; `angle_deg` turns the line about the normal, anticlockwise.
    /// `font` is a font file, the bundled IBM Plex Sans where unset.
    Label {
        object: ObjectId,
        text: String,
        at_mm: Vec3,
        #[serde(default)]
        height_mm: Option<Scalar>,
        #[serde(default)]
        depth_mm: Option<Scalar>,
        #[serde(default = "raised_by_default")]
        raised: bool,
        #[serde(default)]
        follow_surface: bool,
        #[serde(default)]
        angle_deg: Scalar,
        #[serde(default)]
        font: Option<PathBuf>,
    },
    /// Repairs `object`'s mesh by `method`, to the lengths the window's Repair tool starts
    /// with; what was built on its old surface goes.
    Repair {
        object: ObjectId,
        method: RepairMethod,
    },
    /// Turns, bridges and drops what is named on `object`'s mesh: faces from 0, shells from 1,
    /// a bridge as two open edges of two vertices each; drops come last.
    Mend {
        object: ObjectId,
        #[serde(default)]
        flip_faces: Vec<usize>,
        #[serde(default)]
        drop_faces: Vec<usize>,
        #[serde(default)]
        drop_shells: Vec<usize>,
        #[serde(default)]
        bridge: Vec<[[u32; 2]; 2]>,
    },
    /// Presses the picture at `image`, tiled `tile_mm` wide and turned `angle_deg`, into
    /// `object` `depth_mm` deep, as the CLI's `--relief` flags do; `hold_bottom_deg` and
    /// `hold_top_deg` leave the faces that near down or up unmoved, `smoothing_px` blurs the
    /// picture first and `precision` is the lattice's, 0 to 1. Only the object's painted
    /// faces move where any are painted.
    Relief {
        object: ObjectId,
        depth_mm: Scalar,
        image: PathBuf,
        #[serde(default = "tile_mm_by_default")]
        tile_mm: Scalar,
        #[serde(default)]
        angle_deg: Scalar,
        #[serde(default)]
        invert: bool,
        #[serde(default)]
        symmetric: bool,
        #[serde(default)]
        smoothing_px: Scalar,
        #[serde(default)]
        hold_bottom_deg: Scalar,
        #[serde(default)]
        hold_top_deg: Scalar,
        #[serde(default = "precision_by_default")]
        precision: Scalar,
    },
    /// Hollows `object` by `settings`, which it keeps as its own; its supports stay.
    Hollow {
        object: ObjectId,
        settings: HollowState,
    },
    /// Drills a hole of `size` and `kind` into the surface of `object` nearest each of `at_mm`,
    /// as `--drain-at` does; the shell is not rebuilt, the holes are cut when it is baked.
    Drain {
        object: ObjectId,
        at_mm: Vec<Vec3>,
        #[serde(default)]
        size: HoleSize,
        #[serde(default)]
        kind: HoleKind,
    },
}

fn tile_mm_by_default() -> Scalar {
    Tiling::default().tile_mm
}

fn precision_by_default() -> Scalar {
    ReliefSettings::default().precision
}

fn raised_by_default() -> bool {
    true
}

/// How `Edit::Combine` joins its objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Op {
    Union,
    /// The first object less every other.
    Difference,
    Intersection,
}

#[cfg(test)]
mod tests;
